use crate::{
    card::{CardInstance, CardRarity},
    combat::{
        apply_combat_action_with_events, finish_monster_turn_after_player_revival,
        start_player_turn, CombatPhase,
    },
    content::cards::{upgrade_card_instance, ANGER_ID, CLEAVE_ID, PARASITE_ID, SHRUG_IT_OFF_ID},
    content::encounters::{
        generate_beyond_encounter_lists_with_rng, generate_city_encounter_lists_with_rng,
    },
    content::monsters::{DARKLING_ID, TRANSIENT_ID, WRITHING_MASS_ID},
    content::reward_pool::{
        ironclad_reward_card_rarity, random_normal_curse, RewardCardEntry, IRONCLAD_REWARD_ENTRIES,
    },
    content::shop_pool::{
        ironclad_combat_discovery_pool, random_colorless_from_pool, shop_card_content_id,
    },
    ids::{CardId, ContentId},
    map::{generate_target_fixed_map, RoomKind, TargetMapAct},
    potion::{Potion, PotionRarity, FAIRY_HEAL_PERCENT, IRONCLAD_POTION_POOL},
    relic::{
        Relic, RelicKey, RelicTier, BUSTED_CROWN_CARD_REWARD_REDUCTION, QUESTION_CARD_REWARD_BONUS,
        SINGING_BOWL_MAX_HP,
    },
    rng::{RngStream, SimulatorRng, StsRng},
    run::event::enter_spire_heart_event,
    run::potion::{
        apply_combat_card_reward_choice, apply_combat_card_reward_skip,
        apply_discard_select_choice, apply_discard_select_confirm, apply_draw_select_choice,
        apply_draw_select_confirm, apply_exhaust_select_choice, apply_exhaust_select_confirm,
        apply_hand_select_choice, apply_hand_select_confirm, apply_potion_action,
    },
    run::shop::apply_shop_action,
    run::state::{
        RunRngStream, DEFAULT_EVENT_ROOM_MONSTER_CHANCE, DEFAULT_EVENT_ROOM_SHOP_CHANCE,
        DEFAULT_EVENT_ROOM_TREASURE_CHANCE,
    },
    CombatAction, Event, MonsterState, RewardContinuation, RewardScreen, RunAction, RunPhase,
    RunState, SimError, SimResult,
};

/// Source-backed combat reward categories from target `createCombatReward` variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatRewardKind {
    Normal,
    Elite,
    Chest,
    Boss,
}

const REWARD_CARD_COUNT: usize = 3;
const NORMAL_COMBAT_GOLD_MIN: i32 = 10;
const NORMAL_COMBAT_GOLD_MAX: i32 = 20;
const ELITE_COMBAT_GOLD_MIN: i32 = 25;
const ELITE_COMBAT_GOLD_MAX: i32 = 35;
const BOSS_COMBAT_GOLD_BASE: i32 = 100;
const BOSS_COMBAT_GOLD_VARIANCE_MIN: i32 = -5;
const BOSS_COMBAT_GOLD_VARIANCE_MAX: i32 = 5;
const SMALL_CHEST_CHANCE: i32 = 50;
const MEDIUM_CHEST_CHANCE: i32 = 33;
const CHEST_GOLD_CHANCES: [i32; 3] = [50, 35, 50];
const CHEST_GOLD_AMOUNTS: [i32; 3] = [25, 50, 75];
const CHEST_RELIC_COMMON_CHANCES: [i32; 3] = [75, 35, 0];
const CHEST_RELIC_UNCOMMON_CHANCES: [i32; 3] = [25, 50, 75];
const MAX_HAND_SIZE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChestSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TreasureRoomState {
    pub chest_size: ChestSize,
    pub relic_tier: RelicTier,
    pub have_gold: bool,
}

fn target_chest_size(rng: &mut StsRng) -> ChestSize {
    let roll = rng.random_int(99);
    if roll < SMALL_CHEST_CHANCE {
        ChestSize::Small
    } else if roll < SMALL_CHEST_CHANCE + MEDIUM_CHEST_CHANCE {
        ChestSize::Medium
    } else {
        ChestSize::Large
    }
}

fn target_chest_relic_tier(chest_size: ChestSize, roll: i32) -> RelicTier {
    let index = match chest_size {
        ChestSize::Small => 0,
        ChestSize::Medium => 1,
        ChestSize::Large => 2,
    };
    let common_chance = CHEST_RELIC_COMMON_CHANCES[index];
    let uncommon_chance = CHEST_RELIC_UNCOMMON_CHANCES[index];
    if roll < common_chance {
        RelicTier::Common
    } else if roll < common_chance + uncommon_chance {
        RelicTier::Uncommon
    } else {
        RelicTier::Rare
    }
}

fn chest_size_index(chest_size: ChestSize) -> usize {
    match chest_size {
        ChestSize::Small => 0,
        ChestSize::Medium => 1,
        ChestSize::Large => 2,
    }
}

fn target_chest_gold(chest_size: ChestSize, rng: &mut StsRng) -> i32 {
    let base = CHEST_GOLD_AMOUNTS[chest_size_index(chest_size)] as f32;
    rng.random_float_range(base * 0.9, base * 1.1).round() as i32
}

pub fn setup_treasure_room(run: &mut RunState) {
    let mut treasure_rng = run.rng_for_stream(RunRngStream::Treasure);
    let chest_size = target_chest_size(&mut treasure_rng);
    let roll = treasure_rng.random_int(99);
    let have_gold = roll < CHEST_GOLD_CHANCES[chest_size_index(chest_size)];
    let relic_tier = target_chest_relic_tier(chest_size, roll);
    run.store_rng_counter(RunRngStream::Treasure, &treasure_rng);
    run.treasure_room = Some(TreasureRoomState {
        chest_size,
        relic_tier,
        have_gold,
    });
}

/// Prepare the reward overlay that Orrery opens while preserving the shop
/// underneath it. The relic pickup itself adds the five pending card rewards.
pub(crate) fn enter_orrery_reward_screen(run: &mut RunState) {
    run.phase = RunPhase::Reward;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        relic_key_offer: None,
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: false,
        pending_card_reward_count: 0,
    });
}

/// Target Orrery constructs all five CardRewardItems immediately on pickup,
/// consuming card RNG before the player opens any of them.
pub(crate) fn queue_orrery_card_reward_choices(run: &mut RunState) {
    queue_eager_card_reward_choices(run, crate::relic::ORRERY_EAGER_CARD_REWARDS);
}

fn queue_eager_card_reward_choices(run: &mut RunState, count: u8) {
    let mut queued = Vec::with_capacity(count as usize);
    let mut next_card_id = run.next_card_instance_id();
    for _ in 0..count {
        roll_pending_card_reward_choices(run);
        let mut choices =
            std::mem::take(&mut run.reward.as_mut().expect("card reward screen").choices);
        for choice in &mut choices {
            choice.id = CardId::new(next_card_id);
            next_card_id += 1;
        }
        queued.push(choices);
    }
    run.reward
        .as_mut()
        .expect("card reward screen")
        .queued_card_rewards = queued;
}

pub fn roll_event_relic_reward(run: &mut RunState, act: i32) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_relic_tier(&mut relic_rng, act);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    roll_screenless_relic_reward(run, tier)
}

fn roll_screenless_relic_reward(run: &mut RunState, tier: RelicTier) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, false);
    let pools = run.relic_pools.as_mut().expect("relic pools initialized");
    pools.return_random_screenless_relic(tier, &context)
}

const BASE_POTION_DROP_CHANCE: i32 = 40;
const ACT_4: i32 = 4;

/// Legacy fixed reward pool used in early milestones before RNG wiring.
///
/// Fidelity: [`crate::FidelityCategory::LegacyFixed`]. Use only for
/// compatibility tests and old milestone fixtures; production-like seed-start
/// paths should use source-backed reward generation.
#[must_use]
pub fn legacy_fixed_card_reward_choices(next_card_id: u64) -> Vec<CardInstance> {
    [ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID]
        .iter()
        .enumerate()
        .map(|(index, content_id)| {
            CardInstance::new(CardId::new(next_card_id + index as u64), *content_id)
        })
        .collect()
}

/// Compatibility wrapper for [`legacy_fixed_card_reward_choices`].
///
/// Fidelity: [`crate::FidelityCategory::LegacyFixed`].
#[must_use]
pub fn fixed_card_reward_choices(next_card_id: u64) -> Vec<CardInstance> {
    legacy_fixed_card_reward_choices(next_card_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RewardRarityChances {
    rare: i32,
    uncommon: i32,
}

const NORMAL_REWARD_RARITY_CHANCES: RewardRarityChances = RewardRarityChances {
    rare: 3,
    uncommon: 37,
};
const ELITE_REWARD_RARITY_CHANCES: RewardRarityChances = RewardRarityChances {
    rare: 10,
    uncommon: 40,
};
const SHOP_REWARD_RARITY_CHANCES: RewardRarityChances = RewardRarityChances {
    rare: 9,
    uncommon: 37,
};

fn roll_reward_rarity(
    rng: &mut StsRng,
    card_rarity_factor: i32,
    chances: RewardRarityChances,
) -> CardRarity {
    let roll = rng.random_int(99) + card_rarity_factor;
    if roll < chances.rare {
        CardRarity::Rare
    } else if roll < chances.rare + chances.uncommon {
        CardRarity::Uncommon
    } else {
        CardRarity::Common
    }
}

fn roll_placeholder_reward_rarity(rng: &mut SimulatorRng) -> CardRarity {
    let roll = rng.next_usize(RngStream::RewardRarity, "reward_rarity", 140);
    if roll < 100 {
        CardRarity::Common
    } else if roll < 137 {
        CardRarity::Uncommon
    } else {
        CardRarity::Rare
    }
}

fn resolve_rarity(requested: CardRarity, pool: &[RewardCardEntry]) -> CardRarity {
    for rarity in rarity_search_order(requested) {
        if pool.iter().any(|entry| entry.rarity == rarity) {
            return rarity;
        }
    }

    pool.first()
        .map(|entry| entry.rarity)
        .unwrap_or(CardRarity::Common)
}

fn rarity_search_order(requested: CardRarity) -> [CardRarity; 3] {
    match requested {
        CardRarity::Rare => [CardRarity::Rare, CardRarity::Uncommon, CardRarity::Common],
        CardRarity::Uncommon => [CardRarity::Uncommon, CardRarity::Common, CardRarity::Rare],
        CardRarity::Common => [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare],
    }
}

#[must_use]
pub fn placeholder_card_reward_choices(
    rng: &mut SimulatorRng,
    next_card_id: u64,
) -> Vec<CardInstance> {
    let mut pool: Vec<RewardCardEntry> = IRONCLAD_REWARD_ENTRIES.to_vec();
    let mut choices = Vec::with_capacity(REWARD_CARD_COUNT);

    for index in 0..REWARD_CARD_COUNT {
        let requested = roll_placeholder_reward_rarity(rng);
        let rarity = resolve_rarity(requested, &pool);
        let candidate_indices: Vec<usize> = pool
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.rarity == rarity)
            .map(|(index, _)| index)
            .collect();
        let pick = rng.next_usize(
            RngStream::RewardCard,
            "reward_card",
            candidate_indices.len(),
        );
        let entry = pool.remove(candidate_indices[pick]);
        choices.push(CardInstance::new(
            CardId::new(next_card_id + index as u64),
            entry.content_id,
        ));
    }

    choices
}

/// Compatibility wrapper for [`placeholder_card_reward_choices`].
///
/// Fidelity: [`crate::FidelityCategory::Placeholder`]. This uses the
/// simulator-only [`SimulatorRng`] stream and is not a target-game parity claim.
#[must_use]
pub fn card_reward_choices(rng: &mut SimulatorRng, next_card_id: u64) -> Vec<CardInstance> {
    placeholder_card_reward_choices(rng, next_card_id)
}

/// Source-backed target-style Ironclad card reward generation.
///
/// Fidelity: [`crate::FidelityCategory::SourceBacked`]. This preserves the
/// historical `target_*` API while giving new call sites a name that states the
/// parity evidence level.
#[must_use]
pub fn source_backed_card_reward_choices(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
) -> Vec<CardInstance> {
    target_card_reward_choices(rng, card_rarity_factor, next_card_id)
}

#[must_use]
pub fn target_card_reward_choices(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
) -> Vec<CardInstance> {
    target_card_reward_choices_with_count(rng, card_rarity_factor, next_card_id, REWARD_CARD_COUNT)
}

#[must_use]
pub fn target_card_reward_choices_with_count(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
    choice_count: usize,
) -> Vec<CardInstance> {
    target_card_reward_choices_with_count_and_pool(
        rng,
        card_rarity_factor,
        next_card_id,
        choice_count,
        RewardCardPoolKind::Ironclad,
        NORMAL_REWARD_RARITY_CHANCES,
        None,
        true,
    )
}

/// Target `TheLibrary.buttonEffect` rolls rarity again whenever a duplicate
/// card is found, but unlike combat rewards it does not mutate the shared
/// card-rarity factor and does not roll upgrades. Each accepted card is added
/// to the bottom of a target `CardGroup`, which inserts it at index zero and
/// therefore exposes the choices in reverse roll order.
#[must_use]
pub fn target_library_card_choices(
    rng: &mut StsRng,
    card_rarity_factor: i32,
    next_card_id: u64,
    choice_count: usize,
) -> Vec<CardInstance> {
    let mut choices = Vec::with_capacity(choice_count);
    for index in 0..choice_count {
        loop {
            let requested =
                roll_reward_rarity(rng, card_rarity_factor, NORMAL_REWARD_RARITY_CHANCES);
            let rarity = resolve_rarity(requested, IRONCLAD_REWARD_ENTRIES);
            let candidate_indices = IRONCLAD_REWARD_ENTRIES
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.rarity == rarity)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
            let content_id = IRONCLAD_REWARD_ENTRIES[candidate_indices[pick]].content_id;
            if choices
                .iter()
                .any(|choice: &CardInstance| choice.content_id == content_id)
            {
                continue;
            }
            choices.push(CardInstance::new(
                CardId::new(next_card_id + index as u64),
                content_id,
            ));
            break;
        }
    }
    choices.reverse();
    choices
}

#[must_use]
pub fn target_colorless_card_reward_choices_with_count(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
    choice_count: usize,
) -> Vec<CardInstance> {
    target_card_reward_choices_with_count_and_pool(
        rng,
        card_rarity_factor,
        next_card_id,
        choice_count,
        RewardCardPoolKind::Colorless,
        NORMAL_REWARD_RARITY_CHANCES,
        None,
        true,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RewardCardPoolKind {
    Ironclad,
    Colorless,
    AnyColor,
}

#[allow(clippy::too_many_arguments)]
fn target_card_reward_choices_with_count_and_pool(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
    choice_count: usize,
    pool_kind: RewardCardPoolKind,
    rarity_chances: RewardRarityChances,
    forced_requested_rarity: Option<CardRarity>,
    apply_card_rarity_factor: bool,
) -> Vec<CardInstance> {
    let mut choices = Vec::with_capacity(choice_count);

    for index in 0..choice_count {
        let rarity_factor = if apply_card_rarity_factor {
            *card_rarity_factor
        } else {
            0
        };
        let rolled = roll_reward_rarity(rng, rarity_factor, rarity_chances);
        let requested = forced_requested_rarity.unwrap_or(rolled);
        let rarity = match pool_kind {
            RewardCardPoolKind::Ironclad => resolve_rarity(requested, IRONCLAD_REWARD_ENTRIES),
            RewardCardPoolKind::Colorless | RewardCardPoolKind::AnyColor => requested,
        };
        match requested {
            CardRarity::Common => *card_rarity_factor = (*card_rarity_factor - 1).max(-40),
            CardRarity::Rare => *card_rarity_factor = 5,
            CardRarity::Uncommon => {}
        }

        let mut content_id;
        loop {
            content_id = match pool_kind {
                RewardCardPoolKind::Ironclad => {
                    let candidate_indices: Vec<usize> = IRONCLAD_REWARD_ENTRIES
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.rarity == rarity)
                        .map(|(index, _)| index)
                        .collect();
                    let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
                    IRONCLAD_REWARD_ENTRIES[candidate_indices[pick]].content_id
                }
                RewardCardPoolKind::Colorless => random_colorless_from_pool(rng, rarity),
                RewardCardPoolKind::AnyColor => any_color_reward_content_id(rng, rarity),
            };
            if !choices
                .iter()
                .any(|choice: &CardInstance| choice.content_id == content_id)
            {
                break;
            }
        }

        choices.push(CardInstance::new(
            CardId::new(next_card_id + index as u64),
            content_id,
        ));
    }

    choices
}

const ANY_COLOR_COMMON_CARDS: &[&str] = &[
    "ACROBATICS",
    "ANGER",
    "ARMAMENTS",
    "BACKFLIP",
    "BALL_LIGHTNING",
    "BANE",
    "BARRAGE",
    "BEAM_CELL",
    "BLADE_DANCE",
    "BODY_SLAM",
    "BOWLING_BASH",
    "CLASH",
    "TRANQUILITY",
    "CLEAVE",
    "CLOAK_AND_DAGGER",
    "CLOTHESLINE",
    "COLD_SNAP",
    "COMPILE_DRIVER",
    "CONSECRATE",
    "CHARGE_BATTERY",
    "COOLHEADED",
    "CRESCENDO",
    "CRUSH_JOINTS",
    "CUT_THROUGH_FATE",
    "DAGGER_SPRAY",
    "DAGGER_THROW",
    "DEADLY_POISON",
    "DEFLECT",
    "DODGE_AND_ROLL",
    "EMPTY_BODY",
    "EMPTY_FIST",
    "EVALUATE",
    "FLEX",
    "FLURRY_OF_BLOWS",
    "FLYING_KNEE",
    "FLYING_SLEEVES",
    "FOLLOW_UP",
    "CLAW",
    "GO_FOR_THE_EYES",
    "HALT",
    "HAVOC",
    "HEADBUTT",
    "HEAVY_BLADE",
    "HOLOGRAM",
    "IRON_WAVE",
    "JUST_LUCKY",
    "LEAP",
    "OUTMANEUVER",
    "PRESSURE_POINTS",
    "PERFECTED_STRIKE",
    "PIERCING_WAIL",
    "POISONED_STAB",
    "POMMEL_STRIKE",
    "PREPARED",
    "PROSTRATE",
    "PROTECT",
    "QUICK_SLASH",
    "REBOUND",
    "RECURSION",
    "SASH_WHIP",
    "SHRUG_IT_OFF",
    "SLICE",
    "STACK",
    "STEAM_BARRIER",
    "STREAMLINE",
    "SUCKER_PUNCH",
    "SWEEPING_BEAM",
    "SWORD_BOOMERANG",
    "THIRD_EYE",
    "THUNDERCLAP",
    "TRUE_GRIT",
    "TURBO",
    "TWIN_STRIKE",
    "SNEAKY_STRIKE",
    "WARCRY",
    "WILD_STRIKE",
];

const ANY_COLOR_UNCOMMON_CARDS: &[&str] = &[
    "ACCURACY",
    "RUSHDOWN",
    "AGGREGATE",
    "ALL_OUT_ATTACK",
    "AUTO_SHIELDS",
    "BACKSTAB",
    "BANDAGE_UP",
    "BATTLE_TRANCE",
    "BATTLE_HYMN",
    "BLIND",
    "BLIZZARD",
    "BLOOD_FOR_BLOOD",
    "BLOODLETTING",
    "BLUR",
    "BOOT_SEQUENCE",
    "BOUNCING_FLASK",
    "BURNING_PACT",
    "CALCULATED_GAMBLE",
    "CALTROPS",
    "CAPACITOR",
    "CARNAGE",
    "CARVE_REALITY",
    "CATALYST",
    "CHAOS",
    "CHILL",
    "CHOKE",
    "COLLECT",
    "COMBUST",
    "CONCENTRATE",
    "CONCLUDE",
    "CONSUME",
    "CRIPPLING_CLOUD",
    "DARK_EMBRACE",
    "DARK_SHACKLES",
    "DARKNESS",
    "DASH",
    "DECEIVE_REALITY",
    "DEEP_BREATH",
    "DEFRAGMENT",
    "DISARM",
    "DISCOVERY",
    "DISTRACTION",
    "DOOM_AND_GLOOM",
    "DOUBLE_ENERGY",
    "DRAMATIC_ENTRANCE",
    "DROPKICK",
    "DUAL_WIELD",
    "EMPTY_MIND",
    "ENDLESS_AGONY",
    "ENLIGHTENMENT",
    "ENTRENCH",
    "ESCAPE_PLAN",
    "EVISCERATE",
    "EVOLVE",
    "EXPERTISE",
    "FTL",
    "FASTING",
    "FEAR_NO_EVIL",
    "FEEL_NO_PAIN",
    "FINESSE",
    "FINISHER",
    "FIRE_BREATHING",
    "FLAME_BARRIER",
    "FLASH_OF_STEEL",
    "FLECHETTES",
    "FOOTWORK",
    "FORCE_FIELD",
    "FOREIGN_INFLUENCE",
    "FORETHOUGHT",
    "FUSION",
    "GENETIC_ALGORITHM",
    "GHOSTLY_ARMOR",
    "GLACIER",
    "GOOD_INSTINCTS",
    "HEATSINKS",
    "HEEL_HOOK",
    "HELLO_WORLD",
    "HEMOKINESIS",
    "IMPATIENCE",
    "INDIGNATION",
    "INFERNAL_BLADE",
    "INFINITE_BLADES",
    "INFLAME",
    "INNER_PEACE",
    "INTIMIDATE",
    "JACK_OF_ALL_TRADES",
    "LEG_SWEEP",
    "LIKE_WATER",
    "BULLSEYE",
    "LOOP",
    "MADNESS",
    "MASTERFUL_STAB",
    "MEDITATE",
    "MELTER",
    "MENTAL_FORTRESS",
    "METALLICIZE",
    "MIND_BLAST",
    "NIRVANA",
    "NOXIOUS_FUMES",
    "PANACEA",
    "PANIC_BUTTON",
    "PERSEVERANCE",
    "POWER_THROUGH",
    "PRAY",
    "PREDATOR",
    "PUMMEL",
    "PURITY",
    "RAGE",
    "RAMPAGE",
    "REACH_HEAVEN",
    "RECKLESS_CHARGE",
    "RECYCLE",
    "REFLEX",
    "REINFORCED_BODY",
    "REPROGRAM",
    "RIDDLE_WITH_HOLES",
    "RIP_AND_TEAR",
    "RUPTURE",
    "SANCTITY",
    "SANDS_OF_TIME",
    "SCRAPE",
    "SEARING_BLOW",
    "SECOND_WIND",
    "SEEING_RED",
    "SELF_REPAIR",
    "SENTINEL",
    "SETUP",
    "SEVER_SOUL",
    "SHOCKWAVE",
    "SIGNATURE_MOVE",
    "SKEWER",
    "SKIM",
    "SPOT_WEAKNESS",
    "STATIC_DISCHARGE",
    "OVERCLOCK",
    "STORM",
    "STUDY",
    "SUNDER",
    "SWIFT_STRIKE",
    "SWIVEL",
    "TACTICIAN",
    "TALK_TO_THE_HAND",
    "TANTRUM",
    "TEMPEST",
    "TERROR",
    "TRIP",
    "EQUILIBRIUM",
    "UPPERCUT",
    "SIMMERING_FURY",
    "WALLOP",
    "WAVE_OF_THE_HAND",
    "WEAVE",
    "WELL_LAID_PLANS",
    "WHEEL_KICK",
    "WHIRLWIND",
    "WHITE_NOISE",
    "WINDMILL_STRIKE",
    "FORESIGHT",
    "WORSHIP",
    "WREATH_OF_FLAME",
];

const ANY_COLOR_RARE_CARDS: &[&str] = &[
    "A_THOUSAND_CUTS",
    "ADRENALINE",
    "AFTER_IMAGE",
    "ALL_FOR_ONE",
    "ALPHA",
    "AMPLIFY",
    "APOTHEOSIS",
    "BARRICADE",
    "BERSERK",
    "BIASED_COGNITION",
    "BLASPHEMY",
    "BLUDGEON",
    "BRILLIANCE",
    "BRUTALITY",
    "BUFFER",
    "BULLET_TIME",
    "BURST",
    "CHRYSALIS",
    "CONJURE_BLADE",
    "CORE_SURGE",
    "CORPSE_EXPLOSION",
    "CORRUPTION",
    "CREATIVE_AI",
    "DEMON_FORM",
    "DEUS_EX_MACHINA",
    "DEVA_FORM",
    "DEVOTION",
    "DIE_DIE_DIE",
    "DOPPELGANGER",
    "DOUBLE_TAP",
    "ECHO_FORM",
    "ELECTRODYNAMICS",
    "ENVENOM",
    "ESTABLISHMENT",
    "EXHUME",
    "FEED",
    "FIEND_FIRE",
    "FISSION",
    "GLASS_KNIFE",
    "GRAND_FINALE",
    "HAND_OF_GREED",
    "HYPERBEAM",
    "IMMOLATE",
    "IMPERVIOUS",
    "JUDGMENT",
    "JUGGERNAUT",
    "LESSON_LEARNED",
    "LIMIT_BREAK",
    "MACHINE_LEARNING",
    "MAGNETISM",
    "MALAISE",
    "MASTER_OF_STRATEGY",
    "MASTER_REALITY",
    "MAYHEM",
    "METAMORPHOSIS",
    "METEOR_STRIKE",
    "MULTI_CAST",
    "NIGHTMARE",
    "OFFERING",
    "OMNISCIENCE",
    "PANACHE",
    "PHANTASMAL_KILLER",
    "RAGNAROK",
    "RAINBOW",
    "REAPER",
    "REBOOT",
    "SADISTIC_NATURE",
    "SCRAWL",
    "SECRET_TECHNIQUE",
    "SECRET_WEAPON",
    "SEEK",
    "SPIRIT_SHIELD",
    "STORM_OF_STEEL",
    "THE_BOMB",
    "THINKING_AHEAD",
    "THUNDER_STRIKE",
    "TOOLS_OF_THE_TRADE",
    "TRANSMUTATION",
    "UNLOAD",
    "VAULT",
    "ALCHEMIZE",
    "VIOLENCE",
    "WISH",
    "WRAITH_FORM",
];

fn any_color_reward_content_id(rng: &mut StsRng, rarity: CardRarity) -> ContentId {
    let pool = match rarity {
        CardRarity::Common => ANY_COLOR_COMMON_CARDS,
        CardRarity::Uncommon => ANY_COLOR_UNCOMMON_CARDS,
        CardRarity::Rare => ANY_COLOR_RARE_CARDS,
    };
    rng.random_long();
    let pick = rng.random_int((pool.len() - 1) as i32) as usize;
    shop_card_content_id(pool[pick])
}

pub(crate) fn reward_card_choice_count(run: &RunState) -> usize {
    let mut count = REWARD_CARD_COUNT;
    if run.relics.contains(&Relic::QuestionCard) {
        count += QUESTION_CARD_REWARD_BONUS;
    }
    if run.relics.contains(&Relic::BustedCrown) {
        count = count.saturating_sub(BUSTED_CROWN_CARD_REWARD_REDUCTION);
    }
    count.max(1)
}

pub fn target_normal_combat_gold(rng: &mut StsRng) -> i32 {
    rng.random_int_range(NORMAL_COMBAT_GOLD_MIN, NORMAL_COMBAT_GOLD_MAX)
}

pub fn target_elite_combat_gold(rng: &mut StsRng) -> i32 {
    rng.random_int_range(ELITE_COMBAT_GOLD_MIN, ELITE_COMBAT_GOLD_MAX)
}

pub fn target_boss_combat_gold(rng: &mut StsRng) -> i32 {
    BOSS_COMBAT_GOLD_BASE
        + rng.random_int_range(BOSS_COMBAT_GOLD_VARIANCE_MIN, BOSS_COMBAT_GOLD_VARIANCE_MAX)
}

fn combat_gold_offer_with_relics(run: &RunState, amount: i32) -> i32 {
    if run.relics.contains(&Relic::GoldenIdol) {
        amount + (amount as f32 * 0.25).round() as i32
    } else {
        amount
    }
}

pub fn target_relic_tier(rng: &mut StsRng, act: i32) -> RelicTier {
    let common_chance = if act == ACT_4 { 0 } else { 50 };
    let uncommon_chance = if act == ACT_4 { 100 } else { 33 };
    let roll = rng.random_int_range(0, 99);

    if roll < common_chance {
        RelicTier::Common
    } else if roll < common_chance + uncommon_chance {
        RelicTier::Uncommon
    } else {
        RelicTier::Rare
    }
}

pub fn target_elite_relic_tier(rng: &mut StsRng) -> RelicTier {
    let roll = rng.random_int(99);
    if roll < 50 {
        RelicTier::Common
    } else if roll > 82 {
        RelicTier::Rare
    } else {
        RelicTier::Uncommon
    }
}

pub fn target_random_potion(rng: &mut StsRng) -> Potion {
    target_random_potion_with_filter(rng, |_| true)
}

/// Picks directly from the character potion pool with one RNG draw.
///
/// This matches `PotionHelper.getRandomPotion()`. It is distinct from the
/// rarity-first `AbstractDungeon.returnRandomPotion()` behavior modeled by
/// [`target_random_potion`].
pub fn target_uniform_random_potion(rng: &mut StsRng) -> Potion {
    let index = rng.random_int((IRONCLAD_POTION_POOL.len() - 1) as i32) as usize;
    IRONCLAD_POTION_POOL[index]
}

pub fn target_random_combat_potion(rng: &mut StsRng) -> Potion {
    loop {
        let potion = target_random_potion(rng);
        if potion != Potion::FruitJuice {
            return potion;
        }
    }
}

fn target_random_potion_with_filter(
    rng: &mut StsRng,
    allows_potion: impl Fn(Potion) -> bool,
) -> Potion {
    let rarity = match rng.random_int_range(0, 99) {
        roll if roll < 65 => PotionRarity::Common,
        roll if roll < 90 => PotionRarity::Uncommon,
        _ => PotionRarity::Rare,
    };

    loop {
        let index = rng.random_int((IRONCLAD_POTION_POOL.len() - 1) as i32) as usize;
        let potion = IRONCLAD_POTION_POOL[index];
        if potion.rarity() == rarity && allows_potion(potion) {
            return potion;
        }
    }
}

pub fn target_potion_reward_offer(
    rng: &mut StsRng,
    potion_chance: &mut i32,
    reward_count: usize,
    potion_belt_count: usize,
    potion_capacity: usize,
    guaranteed_potion: bool,
) -> Option<Potion> {
    let _ = (potion_belt_count, potion_capacity);

    let mut chance = if guaranteed_potion {
        100
    } else {
        BASE_POTION_DROP_CHANCE + *potion_chance
    };
    if reward_count >= 4 {
        chance = 0;
    }

    if rng.random_int(99) >= chance {
        *potion_chance += 10;
        None
    } else {
        *potion_chance -= 10;
        Some(target_random_potion(rng))
    }
}

pub(crate) fn roll_relic_reward(run: &mut RunState, tier: RelicTier) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, false);
    let pools = run.relic_pools.as_mut().expect("relic pools initialized");
    pools.return_random_relic(tier, &context)
}

fn split_relic_offer(key: RelicKey) -> (Option<Relic>, Option<RelicKey>) {
    let relic_offer = Relic::from_key(key);
    let relic_key_offer = if relic_offer.is_some() {
        None
    } else {
        Some(key)
    };
    (relic_offer, relic_key_offer)
}

fn roll_bonus_relic_offer(run: &mut RunState) -> (Option<Relic>, Option<RelicKey>) {
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_relic_tier(&mut relic_rng, run.current_act);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    split_relic_offer(roll_relic_reward(run, tier))
}

fn roll_matryoshka_bonus_relic_offer(run: &mut RunState) -> (Option<Relic>, Option<RelicKey>) {
    // Matryoshka.onChestOpen uses relicRng.randomBoolean(0.75F): its bonus
    // relic is common on true and uncommon on false. It does not use the
    // normal act relic-tier distribution.
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_matryoshka_relic_tier(&mut relic_rng);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    split_relic_offer(roll_relic_reward(run, tier))
}

fn target_matryoshka_relic_tier(relic_rng: &mut StsRng) -> RelicTier {
    if relic_rng.random_float() < 0.75 {
        RelicTier::Common
    } else {
        RelicTier::Uncommon
    }
}

pub fn enter_relic_reward_screen(run: &mut RunState, kind: CombatRewardKind) {
    run.ensure_ironclad_relic_pools();
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = match kind {
        CombatRewardKind::Elite => target_elite_relic_tier(&mut relic_rng),
        CombatRewardKind::Chest | CombatRewardKind::Boss => {
            target_relic_tier(&mut relic_rng, run.current_act)
        }
        CombatRewardKind::Normal => unreachable!("normal combat rewards do not offer relics"),
    };
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);

    let key = roll_relic_reward(run, tier);
    let (relic_offer, relic_key_offer) = split_relic_offer(key);
    let (pending_relic_offer, pending_relic_key_offer) =
        if kind == CombatRewardKind::Elite && run.relics.contains(&Relic::BlackStar) {
            roll_bonus_relic_offer(run)
        } else {
            (None, None)
        };

    if run.can_gain_potions() {
        let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let potion_capacity = run.potion_capacity();
        let _elite_potion_roll = target_potion_reward_offer(
            &mut potion_rng,
            &mut run.potion_chance,
            2,
            run.potions.len(),
            potion_capacity,
            run.relics.contains(&Relic::WhiteBeastStatue),
        );
        run.store_rng_counter(RunRngStream::Potion, &potion_rng);
    }

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer,
        relic_key_offer,
        pending_relic_offer,
        pending_relic_key_offer,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: false,
        pending_card_reward_count: 0,
    });
}

pub fn enter_boss_relic_reward_screen(run: &mut RunState) {
    let boss_relic_choices = if run.pending_boss_relic_choices.is_empty() {
        let choices = (0..3)
            .map(|_| roll_relic_reward(run, RelicTier::Boss))
            .collect::<Vec<_>>();
        run.pending_boss_relic_choices = choices.clone();
        choices
    } else {
        run.pending_boss_relic_choices.clone()
    };

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.boss_chest_opened = true;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        relic_key_offer: None,
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices,
        card_reward_active: false,
        card_reward_pending: false,
        pending_card_reward_count: 0,
    });
}

pub(crate) fn enter_calling_bell_reward_screen(run: &mut RunState) {
    let common = roll_screenless_relic_reward(run, RelicTier::Common);
    let uncommon = roll_screenless_relic_reward(run, RelicTier::Uncommon);
    let rare = roll_screenless_relic_reward(run, RelicTier::Rare);
    let (relic_offer, relic_key_offer) = split_relic_offer(common);

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer,
        relic_key_offer,
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: vec![uncommon, rare],
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: false,
        pending_card_reward_count: 0,
    });
}

/// Target-style combat entry advances `cardRng` three times before the next reward card roll.
pub fn advance_card_rng_for_combat_entry(run: &mut RunState) {
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    for _ in 0..3 {
        let _ = card_rng.random_int(99);
    }
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
}

/// Consumes the card reward generated and immediately removed by Neow's
/// three-potion reward.
///
/// Target `NeowReward.activate` opens `CombatRewardScreen` after adding the
/// potions. `setupItemReward` constructs a normal card `RewardItem`, including
/// rarity, duplicate-reroll, and upgrade RNG draws, before Neow removes it from
/// the visible rewards.
pub(crate) fn consume_hidden_neow_room_card_reward(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    let choice_count = reward_card_choice_count(run);
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    let mut choices = target_card_reward_choices_with_count_and_pool(
        &mut card_rng,
        &mut run.card_rarity_factor,
        next_card_id,
        choice_count,
        RewardCardPoolKind::Ironclad,
        NORMAL_REWARD_RARITY_CHANCES,
        None,
        true,
    );
    consume_reward_card_upgrade_rolls(&mut card_rng, &mut choices, card_upgraded_chance(run));
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
}

pub fn consume_neow_three_potions_hidden_card_reward(run: &mut RunState) {
    consume_hidden_neow_room_card_reward(run);
}

pub(crate) fn roll_pending_card_reward_choices(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    let choice_count = reward_card_choice_count(run);
    let pool_kind = if run.relics.contains(&Relic::PrismaticShard) {
        RewardCardPoolKind::AnyColor
    } else {
        RewardCardPoolKind::Ironclad
    };
    let rarity_chances = match run.current_room_kind() {
        Some(crate::map::RoomKind::Elite) => ELITE_REWARD_RARITY_CHANCES,
        Some(crate::map::RoomKind::Shop) => SHOP_REWARD_RARITY_CHANCES,
        _ => NORMAL_REWARD_RARITY_CHANCES,
    };
    let apply_card_rarity_factor = true;
    let forced_requested_rarity = if run.current_room_kind() == Some(crate::map::RoomKind::Boss) {
        Some(CardRarity::Rare)
    } else {
        None
    };
    let mut choices = target_card_reward_choices_with_count_and_pool(
        &mut card_rng,
        &mut run.card_rarity_factor,
        next_card_id,
        choice_count,
        pool_kind,
        rarity_chances,
        forced_requested_rarity,
        apply_card_rarity_factor,
    );
    consume_reward_card_upgrade_rolls(&mut card_rng, &mut choices, card_upgraded_chance(run));
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    for choice in &mut choices {
        choice.content_id = run.content_id_after_card_add_relics(choice.content_id);
    }
    run.reward.as_mut().expect("reward screen present").choices = choices;
}

fn preview_obtain_card_reward_choices(run: &mut RunState) {
    let Some(mut choices) = run.reward.as_ref().map(|reward| reward.choices.clone()) else {
        return;
    };
    for choice in &mut choices {
        choice.content_id = run.content_id_after_card_add_relics(choice.content_id);
    }
    run.reward.as_mut().expect("reward screen present").choices = choices;
}

fn card_upgraded_chance(run: &RunState) -> f32 {
    match run.current_act {
        2 if run.ascension >= 12 => 0.125,
        2 => 0.25,
        3 if run.ascension >= 12 => 0.25,
        3 => 0.5,
        _ => 0.0,
    }
}

fn consume_reward_card_upgrade_rolls(
    rng: &mut StsRng,
    choices: &mut [CardInstance],
    upgraded_chance: f32,
) {
    for choice in choices {
        if reward_card_rarity(choice.content_id) == Some(CardRarity::Rare) {
            continue;
        }

        let upgrades = rng.random_float() < upgraded_chance;
        if upgrades {
            if let Some(upgraded) = upgrade_card_instance(*choice) {
                *choice = upgraded;
            }
        }
    }
}

fn reward_card_rarity(content_id: ContentId) -> Option<CardRarity> {
    ironclad_reward_card_rarity(content_id).or_else(|| any_color_reward_card_rarity(content_id))
}

fn any_color_reward_card_rarity(content_id: ContentId) -> Option<CardRarity> {
    if ANY_COLOR_COMMON_CARDS
        .iter()
        .any(|name| shop_card_content_id(name) == content_id)
    {
        Some(CardRarity::Common)
    } else if ANY_COLOR_UNCOMMON_CARDS
        .iter()
        .any(|name| shop_card_content_id(name) == content_id)
    {
        Some(CardRarity::Uncommon)
    } else if ANY_COLOR_RARE_CARDS
        .iter()
        .any(|name| shop_card_content_id(name) == content_id)
    {
        Some(CardRarity::Rare)
    } else {
        None
    }
}

/// Stable target card key for a Prismatic Shard reward card that is not part
/// of the simulator's modeled card registry.
#[must_use]
pub fn any_color_reward_card_key(content_id: ContentId) -> Option<&'static str> {
    ANY_COLOR_COMMON_CARDS
        .iter()
        .chain(ANY_COLOR_UNCOMMON_CARDS.iter())
        .chain(ANY_COLOR_RARE_CARDS.iter())
        .copied()
        .find(|name| shop_card_content_id(name) == content_id)
}

#[must_use]
pub fn any_color_reward_card_key_from_identity(identity: &str) -> Option<&'static str> {
    let normalized = identity
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    ANY_COLOR_COMMON_CARDS
        .iter()
        .chain(ANY_COLOR_UNCOMMON_CARDS.iter())
        .chain(ANY_COLOR_RARE_CARDS.iter())
        .copied()
        .find(|name| {
            name.chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .map(|character| character.to_ascii_lowercase())
                .eq(normalized.chars())
        })
}

pub fn enter_normal_combat_reward_screen(run: &mut RunState) {
    let all_monsters_escaped = run
        .combat
        .as_ref()
        .map(|combat| suppress_gold_for_all_escaped_monsters(&combat.monsters))
        .unwrap_or(false);
    let pending_event_gold_offer = std::mem::take(&mut run.pending_event_combat_gold_offer);
    let pending_event_relic_key_offer = run.pending_event_combat_relic_key_offer.take();
    let gold_offer = if pending_event_gold_offer > 0 {
        combat_gold_offer_with_relics(run, pending_event_gold_offer)
    } else if all_monsters_escaped {
        0
    } else {
        let mut treasure_rng = run.rng_for_stream(RunRngStream::Treasure);
        let gold_offer =
            combat_gold_offer_with_relics(run, target_normal_combat_gold(&mut treasure_rng));
        run.store_rng_counter(RunRngStream::Treasure, &treasure_rng);
        gold_offer
    };
    let (relic_offer, relic_key_offer) = pending_event_relic_key_offer
        .map(split_relic_offer)
        .unwrap_or((None, None));

    let potion_offer = if run.can_gain_potions() {
        let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let potion_capacity = run.potion_capacity();
        let potion_offer = if all_monsters_escaped && !run.relics.contains(&Relic::WhiteBeastStatue)
        {
            let _ = potion_rng.random_int(99);
            run.potion_chance += 10;
            None
        } else {
            target_potion_reward_offer(
                &mut potion_rng,
                &mut run.potion_chance,
                1,
                run.potions.len(),
                potion_capacity,
                run.relics.contains(&Relic::WhiteBeastStatue),
            )
        };
        run.store_rng_counter(RunRngStream::Potion, &potion_rng);
        potion_offer
    } else {
        None
    };

    let pending_card_reward_count = if run.relics.contains(&Relic::PrayerWheel) {
        2
    } else {
        1
    };

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer,
        potion_offers: Vec::new(),
        relic_offer,
        relic_key_offer,
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: true,
        pending_card_reward_count,
    });
    if pending_card_reward_count == 1 {
        roll_pending_card_reward_choices(run);
    } else {
        // CombatRewardScreen constructs both Prayer Wheel RewardItems before either is opened.
        // Their card RNG must therefore be consumed even when the player skips both rewards.
        queue_eager_card_reward_choices(run, pending_card_reward_count);
    }
}

fn suppress_gold_for_all_escaped_monsters(monsters: &[MonsterState]) -> bool {
    !monsters.is_empty()
        && monsters.iter().all(|monster| {
            monster.escaped
                && monster.content_id != DARKLING_ID
                && monster.content_id != TRANSIENT_ID
        })
}

pub fn enter_reward_screen(run: &mut RunState) {
    let stolen_gold_offer = run
        .combat
        .as_ref()
        .map(|combat| {
            combat
                .monsters
                .iter()
                .filter(|monster| !monster.escaped)
                .map(|monster| monster.stolen_gold)
                .sum()
        })
        .unwrap_or(0);
    enter_normal_combat_reward_screen(run);
    if let Some(reward) = run.reward.as_mut() {
        reward.stolen_gold_offer = stolen_gold_offer;
    }
}

pub fn enter_elite_combat_reward_screen(run: &mut RunState) {
    let mut treasure_rng = run.rng_for_stream(RunRngStream::Treasure);
    let gold_offer =
        combat_gold_offer_with_relics(run, target_elite_combat_gold(&mut treasure_rng));
    run.store_rng_counter(RunRngStream::Treasure, &treasure_rng);

    run.ensure_ironclad_relic_pools();
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_elite_relic_tier(&mut relic_rng);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    let key = roll_relic_reward(run, tier);
    let (relic_offer, relic_key_offer) = split_relic_offer(key);
    let (pending_relic_offer, pending_relic_key_offer) = if run.relics.contains(&Relic::BlackStar) {
        roll_bonus_relic_offer(run)
    } else {
        (None, None)
    };

    let potion_offer = if run.can_gain_potions() {
        let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let potion_capacity = run.potion_capacity();
        let potion_offer = target_potion_reward_offer(
            &mut potion_rng,
            &mut run.potion_chance,
            2,
            run.potions.len(),
            potion_capacity,
            run.relics.contains(&Relic::WhiteBeastStatue),
        );
        run.store_rng_counter(RunRngStream::Potion, &potion_rng);
        potion_offer
    } else {
        None
    };

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer,
        potion_offers: Vec::new(),
        relic_offer,
        relic_key_offer,
        pending_relic_offer,
        pending_relic_key_offer,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: true,
        pending_card_reward_count: 1,
    });
    roll_pending_card_reward_choices(run);
}

pub fn enter_boss_combat_reward_screen(run: &mut RunState) {
    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let gold_offer = combat_gold_offer_with_relics(run, target_boss_combat_gold(&mut misc_rng));
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);

    let potion_offer = if run.can_gain_potions() {
        let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let potion_capacity = run.potion_capacity();
        let potion_offer = target_potion_reward_offer(
            &mut potion_rng,
            &mut run.potion_chance,
            1,
            run.potions.len(),
            potion_capacity,
            run.relics.contains(&Relic::WhiteBeastStatue),
        );
        run.store_rng_counter(RunRngStream::Potion, &potion_rng);
        potion_offer
    } else {
        None
    };

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer,
        potion_offers: Vec::new(),
        relic_offer: None,
        relic_key_offer: None,
        pending_relic_offer: None,
        pending_relic_key_offer: None,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: true,
        pending_card_reward_count: 1,
    });
    roll_pending_card_reward_choices(run);
}

fn enter_boss_reward_chest(run: &mut RunState) {
    run.phase = RunPhase::Treasure;
    run.combat = None;
    run.reward = None;
    run.treasure_room = None;
    run.boss_chest_opened = false;
    run.pending_boss_relic_choices.clear();
    run.current_floor += 1;
    run.reinit_room_rngs_for_floor();
}

fn enter_next_act_map(run: &mut RunState) {
    let next_act = run.current_act + 1;
    advance_card_rng_for_dungeon_transition(run);
    run.potion_chance = 0;
    run.event_room_monster_chance = DEFAULT_EVENT_ROOM_MONSTER_CHANCE;
    run.event_room_shop_chance = DEFAULT_EVENT_ROOM_SHOP_CHANCE;
    run.event_room_treasure_chance = DEFAULT_EVENT_ROOM_TREASURE_CHANCE;
    if next_act == 2 {
        run.map = Some(generate_target_fixed_map(
            run.reward_rng_seed as i64,
            TargetMapAct::City,
        ));
        if let Some(map) = run.map.as_mut() {
            map.floor = run.current_floor as u32;
        }
        generate_city_encounters_for_next_act(run);
        run.current_act = 2;
    } else if next_act == 3 {
        run.map = Some(generate_target_fixed_map(
            run.reward_rng_seed as i64,
            TargetMapAct::Beyond,
        ));
        if let Some(map) = run.map.as_mut() {
            map.floor = run.current_floor as u32;
        }
        generate_beyond_encounters_for_next_act(run);
        run.current_act = 3;
    }
    run.phase = RunPhase::Idle;
    run.reward = None;
    run.combat = None;
    run.treasure_room = None;
    run.boss_chest_opened = false;
    run.pending_boss_relic_choices.clear();
    run.current_room_override = None;
    run.normal_combat_count = 0;
    run.elite_combat_count = 0;
    if !run.has_mark_of_bloom() {
        run.player_hp = run.player_max_hp;
    }
}

fn generate_city_encounters_for_next_act(run: &mut RunState) {
    let mut rng = StsRng::new(run.monster_rng_seed as i64);
    crate::content::encounters::advance_exordium_content_generation_rng(&mut rng);
    let (normal, elite) = generate_city_encounter_lists_with_rng(&mut rng);
    run.normal_encounter_list = normal;
    run.elite_encounter_list = elite;
    run.monster_rng_counter = rng.counter();
}

fn generate_beyond_encounters_for_next_act(run: &mut RunState) {
    // Dungeon content generation is replayed from the run seed for each act;
    // combat AI rolls accumulated during Act 2 must not contaminate the Act 3
    // encounter list.
    let mut rng = StsRng::new(run.monster_rng_seed as i64);
    crate::content::encounters::advance_exordium_content_generation_rng(&mut rng);
    let _ = generate_city_encounter_lists_with_rng(&mut rng);
    let (normal, elite) = generate_beyond_encounter_lists_with_rng(&mut rng);
    run.normal_encounter_list = normal;
    run.elite_encounter_list = elite;
    run.monster_rng_counter = rng.counter();
}

fn advance_card_rng_for_dungeon_transition(run: &mut RunState) {
    match run.card_rng_counter {
        1..=249 => run.card_rng_counter = 250,
        251..=499 => run.card_rng_counter = 500,
        501..=749 => run.card_rng_counter = 750,
        _ => {}
    }
}

pub fn enter_elite_relic_reward_screen(run: &mut RunState) {
    enter_relic_reward_screen(run, CombatRewardKind::Elite);
}

pub fn enter_chest_relic_reward_screen(run: &mut RunState) {
    if run.treasure_room.is_none() {
        setup_treasure_room(run);
    }
    apply_cursed_key_chest_curse(run);
    let treasure_room = *run
        .treasure_room
        .as_ref()
        .expect("treasure room must be initialized before opening chest");
    let tier = treasure_room.relic_tier;
    let gold_offer = if treasure_room.have_gold {
        let mut treasure_rng = run.rng_for_stream(RunRngStream::Treasure);
        let amount = target_chest_gold(treasure_room.chest_size, &mut treasure_rng);
        run.store_rng_counter(RunRngStream::Treasure, &treasure_rng);
        amount
    } else {
        0
    };
    let (bonus_relic_offer, bonus_relic_key_offer) =
        if run.relics.contains(&Relic::Matryoshka) && run.matryoshka_chests_opened < 2 {
            run.matryoshka_chests_opened += 1;
            roll_matryoshka_bonus_relic_offer(run)
        } else {
            (None, None)
        };
    // AbstractChest.open invokes relic onChestOpen hooks before adding the
    // chest's own relic reward. Matryoshka therefore consumes relic RNG and
    // removes its relic from the pool before the normal chest relic is rolled.
    let key = roll_relic_reward(run, tier);
    let (chest_relic_offer, chest_relic_key_offer) = split_relic_offer(key);
    // Matryoshka's extra reward is inserted before the chest's normal relic
    // in the target reward list.
    let (
        mut relic_offer,
        mut relic_key_offer,
        mut pending_relic_offer,
        mut pending_relic_key_offer,
    ) = if bonus_relic_offer.is_some() || bonus_relic_key_offer.is_some() {
        (
            bonus_relic_offer,
            bonus_relic_key_offer,
            chest_relic_offer,
            chest_relic_key_offer,
        )
    } else {
        (chest_relic_offer, chest_relic_key_offer, None, None)
    };
    if pending_relic_offer.is_some_and(is_bottled_relic_offer)
        || pending_relic_key_offer.is_some_and(is_bottled_relic_key_offer)
    {
        std::mem::swap(&mut relic_offer, &mut pending_relic_offer);
        std::mem::swap(&mut relic_key_offer, &mut pending_relic_key_offer);
    }

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::None,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer,
        relic_key_offer,
        pending_relic_offer,
        pending_relic_key_offer,
        queued_relic_key_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_active: false,
        card_reward_pending: false,
        pending_card_reward_count: 0,
    });
}

fn is_bottled_relic_offer(relic: Relic) -> bool {
    matches!(
        relic,
        Relic::BottledFlame | Relic::BottledLightning | Relic::BottledTornado
    )
}

fn is_bottled_relic_key_offer(key: RelicKey) -> bool {
    matches!(
        key,
        RelicKey::BottledFlame | RelicKey::BottledLightning | RelicKey::BottledTornado
    )
}

fn apply_cursed_key_chest_curse(run: &mut RunState) {
    if !run.relics.contains(&Relic::CursedKey) {
        return;
    }

    // Target `CardLibrary.getCurse()` samples with `AbstractDungeon.cardRng`.
    // This is the persistent reward-card stream, not the per-combat
    // `cardRandomRng` stream.
    let mut rng = run.rng_for_stream(RunRngStream::CardReward);
    let curse = random_normal_curse(&mut rng);
    run.store_rng_counter(RunRngStream::CardReward, &rng);
    run.gain_deck_card(curse);
}

pub fn apply_combat_action_on_run(run: &RunState, action: CombatAction) -> SimResult<RunState> {
    run.validate()?;

    if run.phase != RunPhase::Combat {
        return Err(SimError::IllegalAction(
            "combat actions require combat phase",
        ));
    }

    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::InvalidState("combat state is missing"))?;

    let mut combat_for_action = combat.clone();
    combat_for_action.relics = run.relics.clone();
    combat_for_action.relic_counters.lizard_tail_available =
        run.relics.contains(&Relic::LizardTail) && !run.lizard_tail_used;
    combat_for_action.relic_counters.fairy_consumed = false;
    combat_for_action.relic_counters.fairy_heal_percent = if run
        .occupied_potion_slots()
        .iter()
        .any(|(_, potion)| *potion == Potion::Fairy)
        && !run.has_mark_of_bloom()
    {
        FAIRY_HEAL_PERCENT
            * if run.relics.contains(&Relic::SacredBark) {
                2
            } else {
                1
            }
    } else {
        0
    };

    let transition = apply_combat_action_with_events(&combat_for_action, action)?;
    let mut next_combat = transition.state;
    let mut next = run.clone();
    if next_combat.relic_counters.fairy_consumed {
        if let Some((slot, _)) = next
            .occupied_potion_slots()
            .into_iter()
            .find(|(_, potion)| *potion == Potion::Fairy)
        {
            next.take_potion_slot(slot)
                .expect("consumed fairy potion was present before combat transition");
        }
    }
    if next.relics.contains(&Relic::LizardTail) && !next_combat.relic_counters.lizard_tail_available
    {
        next.lizard_tail_used = true;
    }
    apply_looter_theft_to_run_gold(&mut next, &combat_for_action, &mut next_combat);
    apply_combat_gold_gain_to_run(&mut next, &combat_for_action, &mut next_combat);
    apply_writhing_mass_mega_debuff_to_run(&mut next, &combat_for_action, &mut next_combat);
    sync_ritual_dagger_damage_to_deck(&mut next, &next_combat);
    next.store_rng_counter(RunRngStream::CardRandom, &next_combat.rng.card_random_rng);
    let dead_branch_placement = if matches!(action, CombatAction::EndTurn) {
        DeadBranchPlacement::FrontOfHand
    } else {
        DeadBranchPlacement::BackOfHand
    };
    if matches!(action, CombatAction::EndTurn) {
        apply_dead_branch_for_exhaust_log(
            &mut next,
            &mut next_combat,
            &transition.event_log,
            dead_branch_placement,
        );
    }
    next_combat.rng.card_random_rng = next.card_random_rng();
    let revived = apply_fairy_if_lethal(&mut next, &mut next_combat);
    if revived
        && matches!(action, CombatAction::EndTurn)
        && next_combat.phase == CombatPhase::WaitingForPlayer
        && next_combat.monsters.iter().any(|monster| monster.alive)
    {
        finish_monster_turn_after_player_revival(&mut next_combat);
        start_player_turn(&mut next_combat);
    }
    next.combat = Some(next_combat.clone());
    next.player_hp = next_combat.player.hp;
    next.player_max_hp = next_combat.player.max_hp;
    if next.relics.contains(&Relic::IncenseBurner) {
        next.incense_burner_counter = next_combat.relic_counters.incense_burner_counter;
    }
    if next.relics.contains(&Relic::PenNib) {
        next.pen_nib_attacks_played = next_combat.relic_counters.pen_nib_attacks_played;
    }
    if next.relics.contains(&Relic::InkBottle) {
        next.ink_bottle_cards_played = next_combat.relic_counters.ink_bottle_cards_played;
    }
    if next.relics.contains(&Relic::HappyFlower) {
        next.happy_flower_turns = next_combat.relic_counters.happy_flower_turns;
    }
    if next.relics.contains(&Relic::Sundial) {
        next.sundial_shuffles = next_combat.relic_counters.sundial_shuffles;
    }
    if next.relics.contains(&Relic::Nunchaku) {
        next.nunchaku_attacks_played = next_combat.relic_counters.nunchaku_attacks_played;
    }

    if next_combat.phase == CombatPhase::Won {
        if next.current_room_kind() == Some(crate::map::RoomKind::Boss) {
            enter_boss_combat_reward_screen(&mut next);
        } else if next.current_room_kind() == Some(crate::map::RoomKind::Elite) {
            enter_elite_combat_reward_screen(&mut next);
        } else {
            enter_reward_screen(&mut next);
        }
    }

    Ok(next)
}

fn apply_writhing_mass_mega_debuff_to_run(
    run: &mut RunState,
    before: &crate::combat::CombatState,
    after: &mut crate::combat::CombatState,
) {
    let triggered = after.monsters.iter().any(|monster| {
        monster.content_id == WRITHING_MASS_ID
            && monster.has_siphoned
            && before
                .monsters
                .iter()
                .find(|before_monster| before_monster.id == monster.id)
                .is_none_or(|before_monster| !before_monster.has_siphoned)
    });
    if !triggered {
        return;
    }

    // AddCardToDeckAction mutates the master deck during combat. Keep the run
    // and combat player views aligned so card-obtain relics apply immediately.
    run.player_hp = after.player.hp;
    run.player_max_hp = after.player.max_hp;
    run.gain_deck_card(PARASITE_ID);
    after.player.hp = run.player_hp;
    after.player.max_hp = run.player_max_hp;
}

fn sync_ritual_dagger_damage_to_deck(run: &mut RunState, combat: &crate::combat::CombatState) {
    for deck_card in &mut run.deck {
        let Some(combat_card) = combat
            .piles
            .hand
            .iter()
            .chain(combat.piles.draw_pile.iter())
            .chain(combat.piles.discard_pile.iter())
            .chain(combat.piles.exhaust_pile.iter())
            .find(|card| card.id == deck_card.id)
        else {
            continue;
        };
        if combat_card.ritual_dagger_damage_bonus > deck_card.ritual_dagger_damage_bonus {
            deck_card.ritual_dagger_damage_bonus = combat_card.ritual_dagger_damage_bonus;
        }
    }
}

fn apply_combat_gold_gain_to_run(
    run: &mut RunState,
    before: &crate::combat::CombatState,
    after: &mut crate::combat::CombatState,
) {
    let delta = (after.combat_gold_gained - before.combat_gold_gained).max(0);
    if delta > 0 {
        run.gain_gold(delta);
    }
    after.combat_gold_gained = before.combat_gold_gained + delta;
}

fn apply_looter_theft_to_run_gold(
    run: &mut RunState,
    before: &crate::combat::CombatState,
    after: &mut crate::combat::CombatState,
) {
    for monster in &mut after.monsters {
        let before_stolen = before
            .monsters
            .iter()
            .find(|before_monster| before_monster.id == monster.id)
            .map(|before_monster| before_monster.stolen_gold)
            .unwrap_or(0);
        let delta = (monster.stolen_gold - before_stolen).max(0);
        if delta == 0 {
            continue;
        }
        let actual = delta.min(run.gold);
        run.gold -= actual;
        monster.stolen_gold = before_stolen + actual;
    }
}

fn apply_dead_branch_for_exhaust_log(
    run: &mut RunState,
    combat: &mut crate::combat::CombatState,
    event_log: &[crate::InternalAction],
    placement: DeadBranchPlacement,
) {
    let exhaust_count = event_log
        .iter()
        .filter(|action| matches!(action, crate::InternalAction::CardExhausted { .. }))
        .count();
    apply_dead_branch_for_exhaust_count_with_placement(run, combat, exhaust_count, placement);
}

pub(crate) fn apply_dead_branch_for_exhaust_count(
    run: &mut RunState,
    combat: &mut crate::combat::CombatState,
    exhaust_count: usize,
) {
    apply_dead_branch_for_exhaust_count_with_placement(
        run,
        combat,
        exhaust_count,
        DeadBranchPlacement::BackOfHand,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadBranchPlacement {
    BackOfHand,
    FrontOfHand,
}

fn apply_dead_branch_for_exhaust_count_with_placement(
    run: &mut RunState,
    combat: &mut crate::combat::CombatState,
    exhaust_count: usize,
    placement: DeadBranchPlacement,
) {
    if exhaust_count == 0
        || !run.relics.contains(&Relic::DeadBranch)
        || !combat.monsters.iter().any(|monster| monster.alive)
    {
        return;
    }

    let pool = dead_branch_card_pool();
    let mut rng = run.card_random_rng();
    let available_hand_slots = MAX_HAND_SIZE.saturating_sub(combat.piles.hand.len());
    let mut generated = Vec::with_capacity(exhaust_count);
    for next_id in (combat.next_card_instance_id()..).take(exhaust_count) {
        let index = rng.random_int((pool.len() - 1) as i32) as usize;
        let mut card = CardInstance::new(CardId::new(next_id), pool[index]);
        card.combat_only = true;
        if generated.len() < available_hand_slots {
            generated.push(card);
        } else {
            combat.piles.discard_pile.push(card);
        }
    }
    match placement {
        DeadBranchPlacement::BackOfHand => combat.piles.hand.extend(generated),
        DeadBranchPlacement::FrontOfHand => {
            for card in generated.into_iter().rev() {
                combat.piles.hand.insert(0, card);
            }
        }
    }
    run.store_rng_counter(RunRngStream::CardRandom, &rng);
    combat.rng.card_random_rng = rng;
}

fn dead_branch_card_pool() -> Vec<ContentId> {
    ironclad_combat_discovery_pool().to_vec()
}

fn apply_fairy_if_lethal(run: &mut RunState, combat: &mut crate::combat::CombatState) -> bool {
    if combat.player.hp > 0 && combat.phase != CombatPhase::Lost {
        return false;
    }

    if run.has_mark_of_bloom() {
        return false;
    }

    if run.relics.contains(&Relic::LizardTail) && !run.lizard_tail_used {
        run.lizard_tail_used = true;
        combat.player.hp =
            (combat.player.max_hp * crate::relic::LIZARD_TAIL_HEAL_PERCENT / 100).max(1);
        combat.phase = CombatPhase::WaitingForPlayer;
        return true;
    }

    let Some((slot, _)) = run
        .occupied_potion_slots()
        .into_iter()
        .find(|(_, potion)| *potion == Potion::Fairy)
    else {
        return false;
    };

    run.take_potion_slot(slot)
        .expect("fairy potion slot was found before consuming");
    let multiplier = if run.relics.contains(&Relic::SacredBark) {
        2
    } else {
        1
    };
    combat.player.hp = (combat.player.max_hp * FAIRY_HEAL_PERCENT * multiplier / 100).max(1);
    combat.phase = CombatPhase::WaitingForPlayer;
    true
}

pub fn apply_run_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    run.validate()?;

    let next = match action {
        RunAction::OpenChest => apply_treasure_action(run, action),
        RunAction::Proceed if run.phase == RunPhase::Reward => apply_reward_action(run, action),
        RunAction::Proceed if run.phase == RunPhase::Shop => apply_shop_action(run, action),
        RunAction::Proceed => apply_treasure_action(run, action),
        RunAction::BuyShopCard { .. }
        | RunAction::BuyShopRelic { .. }
        | RunAction::BuyShopPotion { .. }
        | RunAction::EnterShop
        | RunAction::LeaveShop
        | RunAction::OpenShopRemove => apply_shop_action(run, action),
        RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
            apply_potion_action(run, action)
        }
        RunAction::ChooseCombatCardReward { index } => apply_combat_card_reward_choice(run, index),
        RunAction::SkipCombatCardReward => apply_combat_card_reward_skip(run),
        RunAction::ChooseHandSelect { index } => apply_hand_select_choice(run, index),
        RunAction::ConfirmHandSelect => apply_hand_select_confirm(run),
        RunAction::ChooseDrawSelect { index } => apply_draw_select_choice(run, index),
        RunAction::ConfirmDrawSelect => apply_draw_select_confirm(run),
        RunAction::ChooseDiscardSelect { index } => apply_discard_select_choice(run, index),
        RunAction::ConfirmDiscardSelect => apply_discard_select_confirm(run),
        RunAction::ChooseExhaustSelect { index } => apply_exhaust_select_choice(run, index),
        RunAction::ConfirmExhaustSelect => apply_exhaust_select_confirm(run),
        _ => apply_reward_action(run, action),
    }?;
    next.validate()?;
    Ok(next)
}

pub fn validate_treasure_action(run: &RunState, action: RunAction) -> SimResult<()> {
    run.validate()?;

    if run.phase != RunPhase::Treasure {
        return Err(SimError::IllegalAction(
            "treasure actions require treasure phase",
        ));
    }
    match action {
        RunAction::OpenChest => {
            if run.current_room_kind() == Some(RoomKind::Boss)
                && run.treasure_room.is_none()
                && !run.boss_chest_opened
            {
                return Ok(());
            }
            if run.treasure_room.is_some() {
                Ok(())
            } else {
                Err(SimError::InvalidState("treasure room is missing"))
            }
        }
        RunAction::Proceed => {
            if run.current_room_kind() == Some(RoomKind::Boss)
                && run.reward.is_none()
                && run.boss_chest_opened
            {
                Ok(())
            } else {
                Err(SimError::IllegalAction("cannot proceed from treasure"))
            }
        }
        _ => Err(SimError::IllegalAction("not a treasure action")),
    }
}

pub fn apply_treasure_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    validate_treasure_action(run, action)?;
    let mut next = run.clone();
    match action {
        RunAction::OpenChest => {
            if next.current_room_kind() == Some(RoomKind::Boss) && next.treasure_room.is_none() {
                enter_boss_relic_reward_screen(&mut next);
            } else {
                enter_chest_relic_reward_screen(&mut next);
            }
            Ok(next)
        }
        RunAction::Proceed => {
            enter_next_act_map(&mut next);
            Ok(next)
        }
        _ => unreachable!("validated treasure action"),
    }
}

fn apply_reward_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    run.validate_reward_action(action)?;

    let mut next = run.clone();
    match action {
        RunAction::SkipReward => {
            let is_boss_room = next.current_room_kind() == Some(RoomKind::Boss);
            let reward = next.reward.as_mut().expect("validated reward screen");
            if reward.card_reward_active {
                reward.choices.clear();
                reward.card_reward_active = false;
                reward.consume_pending_card_reward();
                return_to_event_if_reward_empty(&mut next);
            } else if is_boss_room && !reward.boss_relic_choices.is_empty() {
                next.phase = RunPhase::Treasure;
                next.reward = None;
                next.boss_chest_opened = false;
            } else if is_boss_room
                && reward.boss_relic_choices.is_empty()
                && reward.pending_relic_offer.is_none()
                && reward.pending_relic_key_offer.is_none()
                && reward.queued_relic_key_offers.is_empty()
            {
                enter_boss_reward_chest(&mut next);
            } else if next.event.is_some() {
                next.phase = RunPhase::Event;
                next.reward = None;
            } else {
                next.phase = RunPhase::Idle;
                next.reward = None;
            }
        }
        RunAction::CloseCardReward => {
            let return_to_rest = next
                .reward
                .as_ref()
                .is_some_and(|reward| reward.continuation == RewardContinuation::Rest);
            let reward = next.reward.as_mut().expect("validated reward screen");
            reward.card_reward_active = false;
            if return_to_rest {
                reward.choices.clear();
                reward.consume_pending_card_reward();
                return_to_event_if_reward_empty(&mut next);
            }
        }
        RunAction::TakeCardReward { card_id } => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let choice = reward
                .choices
                .iter()
                .find(|choice| choice.id == card_id)
                .copied()
                .expect("validated reward card");
            reward.choices.clear();
            reward.card_reward_active = false;
            reward.consume_pending_card_reward();
            next.add_deck_card(choice);
            return_to_event_if_reward_empty(&mut next);
        }
        RunAction::TakeSingingBowlReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            reward.choices.clear();
            reward.card_reward_active = false;
            reward.consume_pending_card_reward();
            next.player_max_hp += SINGING_BOWL_MAX_HP;
            next.player_hp += SINGING_BOWL_MAX_HP;
            return_to_event_if_reward_empty(&mut next);
        }
        RunAction::TakeGoldReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let gold_offer = reward.gold_offer;
            reward.gold_offer = 0;
            next.gain_gold(gold_offer);
        }
        RunAction::TakeStolenGoldReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let stolen_gold_offer = reward.stolen_gold_offer;
            reward.stolen_gold_offer = 0;
            next.gain_gold(stolen_gold_offer);
        }
        RunAction::TakePotionReward { index } => {
            let potion = {
                let reward = next.reward.as_mut().expect("validated reward screen");
                if !reward.potion_offers.is_empty() {
                    reward.potion_offers.remove(index)
                } else {
                    reward.potion_offer.take().expect("validated potion offer")
                }
            };
            next.gain_potion(potion)?;
        }
        RunAction::TakeRelicReward => {
            let (relic_offer, relic_key_offer) = {
                let reward = next.reward.as_mut().expect("validated reward screen");
                (reward.relic_offer.take(), reward.relic_key_offer.take())
            };
            if let Some(relic) = relic_offer {
                next.gain_relic(relic);
            } else if let Some(key) = relic_key_offer {
                next.gain_relic_key(key);
            }
            if next.phase == RunPhase::Reward && next.card_grid.is_none() {
                advance_pending_relic_offer(&mut next);
            }
            let boss_calling_bell_rewards_complete = next.boss_chest_opened
                && next.reward.as_ref().is_some_and(|reward| {
                    reward.relic_offer.is_none()
                        && reward.relic_key_offer.is_none()
                        && reward.pending_relic_offer.is_none()
                        && reward.pending_relic_key_offer.is_none()
                        && reward.queued_relic_key_offers.is_empty()
                });
            if boss_calling_bell_rewards_complete {
                next.phase = RunPhase::Treasure;
                next.reward = None;
            }
        }
        RunAction::ChooseBossRelicReward { index } => {
            let key = {
                let reward = next.reward.as_mut().expect("validated reward screen");
                let key = reward.boss_relic_choices[index];
                reward.boss_relic_choices.clear();
                key
            };
            next.pending_boss_relic_choices.clear();
            next.gain_relic_key(key);
            next.phase = RunPhase::Treasure;
            next.reward = None;
        }
        RunAction::Proceed => {
            if next.current_act == 3 && next.current_room_kind() == Some(RoomKind::Boss) {
                enter_spire_heart_event(&mut next);
                return Ok(next);
            }
            let neow_leave = next
                .event
                .as_ref()
                .is_some_and(|event| event.event == Event::Neow && event.stage == 2);
            next.phase = if neow_leave {
                RunPhase::Idle
            } else {
                RunPhase::Event
            };
            next.reward = None;
            if neow_leave {
                next.event = None;
            }
        }
        RunAction::OpenChest => {
            unreachable!("validated reward action")
        }
        RunAction::OpenCardReward => {
            if next.reward.as_ref().is_some_and(|reward| {
                reward.choices.is_empty() && reward.pending_card_reward_count() > 0
            }) {
                let queued = next.reward.as_mut().and_then(|reward| {
                    (!reward.queued_card_rewards.is_empty())
                        .then(|| reward.queued_card_rewards.remove(0))
                });
                if let Some(choices) = queued {
                    next.reward.as_mut().expect("reward screen present").choices = choices;
                } else {
                    roll_pending_card_reward_choices(&mut next);
                }
            }
            preview_obtain_card_reward_choices(&mut next);
            next.reward
                .as_mut()
                .expect("validated reward screen")
                .card_reward_active = true;
        }
        RunAction::SkipPotionReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            reward.potion_offer = None;
            reward.potion_offers.clear();
        }
        RunAction::BuyShopCard { .. }
        | RunAction::BuyShopRelic { .. }
        | RunAction::BuyShopPotion { .. }
        | RunAction::EnterShop
        | RunAction::LeaveShop
        | RunAction::OpenShopRemove => {
            unreachable!("validated reward action")
        }
        RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
            unreachable!("validated reward action")
        }
        RunAction::ChooseCombatCardReward { .. } | RunAction::SkipCombatCardReward => {
            unreachable!("validated reward action")
        }
        RunAction::ChooseHandSelect { .. } | RunAction::ConfirmHandSelect => {
            unreachable!("validated reward action")
        }
        RunAction::ChooseDrawSelect { .. } | RunAction::ConfirmDrawSelect => {
            unreachable!("validated reward action")
        }
        RunAction::ChooseDiscardSelect { .. } | RunAction::ConfirmDiscardSelect => {
            unreachable!("validated reward action")
        }
        RunAction::ChooseExhaustSelect { .. } | RunAction::ConfirmExhaustSelect => {
            unreachable!("validated reward action")
        }
    }

    Ok(next)
}

fn return_to_event_if_reward_empty(run: &mut RunState) {
    let Some(reward) = run.reward.as_ref() else {
        return;
    };
    if reward.card_reward_active
        || reward.card_reward_pending
        || reward.pending_card_reward_count() > 0
        || !reward.choices.is_empty()
        || reward.gold_offer > 0
        || reward.stolen_gold_offer > 0
        || reward.potion_offer.is_some()
        || reward.relic_offer.is_some()
        || reward.relic_key_offer.is_some()
        || reward.pending_relic_offer.is_some()
        || reward.pending_relic_key_offer.is_some()
        || !reward.queued_relic_key_offers.is_empty()
        || !reward.boss_relic_choices.is_empty()
    {
        return;
    }
    let continuation = reward.continuation;
    if continuation == RewardContinuation::Rest {
        run.phase = RunPhase::Rest;
        run.reward = None;
    } else if run.shop.is_some() && run.shop_merchant_open {
        run.phase = RunPhase::Shop;
        run.reward = None;
    } else if run.event.is_some() {
        run.phase = RunPhase::Event;
        run.reward = None;
    }
}

pub(crate) fn reward_is_empty(reward: &RewardScreen) -> bool {
    !reward.card_reward_active
        && !reward.card_reward_pending
        && reward.pending_card_reward_count() == 0
        && reward.choices.is_empty()
        && reward.queued_card_rewards.is_empty()
        && reward.gold_offer == 0
        && reward.stolen_gold_offer == 0
        && reward.potion_offer.is_none()
        && reward.potion_offers.is_empty()
        && reward.relic_offer.is_none()
        && reward.relic_key_offer.is_none()
        && reward.pending_relic_offer.is_none()
        && reward.pending_relic_key_offer.is_none()
        && reward.queued_relic_key_offers.is_empty()
        && reward.boss_relic_choices.is_empty()
}

pub(crate) fn advance_pending_relic_offer(run: &mut RunState) {
    let Some(reward) = run.reward.as_mut() else {
        return;
    };

    if reward.pending_relic_offer.is_some() || reward.pending_relic_key_offer.is_some() {
        reward.relic_offer = reward.pending_relic_offer.take();
        reward.relic_key_offer = if reward.relic_offer.is_some() {
            reward.pending_relic_key_offer = None;
            None
        } else {
            reward.pending_relic_key_offer.take()
        };
        return;
    }

    let Some(next_key) = reward.queued_relic_key_offers.first().copied() else {
        reward.relic_offer = None;
        reward.relic_key_offer = None;
        return;
    };
    reward.queued_relic_key_offers.remove(0);
    let (relic_offer, relic_key_offer) = split_relic_offer(next_key);
    reward.relic_offer = relic_offer;
    reward.relic_key_offer = if reward.relic_offer.is_some() {
        reward.pending_relic_key_offer = None;
        None
    } else {
        relic_key_offer
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        content::cards::{
            DUAL_WIELD_ID, FIRE_BREATHING_ID, HEADBUTT_ID, HEAVY_BLADE_ID, METALLICIZE_ID,
            PARASITE_ID, POMMEL_STRIKE_ID, POWER_THROUGH_ID, SHOCKWAVE_PLUS_ID, SPOT_WEAKNESS_ID,
            STRIKE_R_ID, SWIFT_STRIKE_ID, THUNDERCLAP_ID, WARCRY_ID, WHIRLWIND_ID,
        },
        content::monsters::{monster_state, DARKLING_ID, WRITHING_MASS_A0},
        run::{
            neow::{generate_neow_colorless_reward, NeowRewardType},
            RunState,
        },
        CardId, CardInstance, CombatAction, CombatState, MonsterId, Relic, RelicKey, RoomKind,
    };

    fn reward_choice_ids(run: &RunState) -> Vec<ContentId> {
        run.reward
            .as_ref()
            .expect("reward screen")
            .choices
            .iter()
            .map(|choice| choice.content_id)
            .collect()
    }

    #[test]
    fn cursed_key_uses_persistent_card_rng_on_session31_seed() {
        let seed = (-7_812_685_662_221_499_508_i64) as u64;
        let mut run = RunState::placeholder_seeded_ironclad(seed, 0);
        run.reward_rng_seed = seed;
        run.card_rng_counter = 272;
        run.card_random_rng_counter = 19;
        run.relics.push(Relic::CursedKey);
        let deck_len = run.deck.len();

        apply_cursed_key_chest_curse(&mut run);

        assert_eq!(run.deck.len(), deck_len + 1);
        assert_eq!(
            run.deck.last().map(|card| card.content_id),
            Some(crate::content::cards::WRITHE_ID)
        );
        assert_eq!(run.card_rng_counter, 273);
        assert_eq!(run.card_random_rng_counter, 19);
    }

    #[test]
    fn matryoshka_uses_its_own_float_tier_roll_on_session_1181_seed() {
        let mut relic_rng = StsRng::with_counter(7_708_489_759_596_451_588_i64, 10);

        assert_eq!(
            target_matryoshka_relic_tier(&mut relic_rng),
            RelicTier::Uncommon
        );
        assert_eq!(relic_rng.counter(), 11);
    }

    #[test]
    fn boss_calling_bell_relics_return_to_treasure_proceed() {
        let mut run = RunState::placeholder_seeded_ironclad(7, 0);
        run.boss_chest_opened = true;
        enter_calling_bell_reward_screen(&mut run);

        for _ in 0..3 {
            run = apply_run_action(&run, RunAction::TakeRelicReward)
                .expect("Calling Bell relic can be collected");
        }

        assert_eq!(run.phase, RunPhase::Treasure);
        assert!(run.reward.is_none());
    }

    #[test]
    fn final_boss_inaccessible_reward_proceeds_to_spire_heart() {
        let mut run = RunState::placeholder_seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        run.current_act = 3;
        run.current_floor = 50;
        run.current_room_override = Some(RoomKind::Boss);
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 100,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            relic_key_offer: None,
            pending_relic_offer: None,
            pending_relic_key_offer: None,
            queued_relic_key_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_active: false,
            card_reward_pending: false,
            pending_card_reward_count: 0,
        });

        let next = apply_run_action(&run, RunAction::Proceed)
            .expect("final boss victory can enter the Spire Heart event");

        assert_eq!(next.phase, RunPhase::Event);
        assert_eq!(next.current_floor, 51);
        assert_eq!(next.current_room_kind(), Some(RoomKind::Victory));
        assert_eq!(next.gold, run.gold);
        assert!(next.reward.is_none());
        let event = next.event.as_ref().expect("Spire Heart event screen");
        assert_eq!(event.event, Event::SpireHeart);
        assert_eq!(event.stage, 0);
        assert_eq!(event.choices.len(), 1);
        assert_eq!(event.choices[0].label, "Continue");

        let json = serde_json::to_string(&next).expect("serialize Spire Heart run state");
        let restored: RunState =
            serde_json::from_str(&json).expect("restore Spire Heart run state");
        assert_eq!(restored, next);

        let mut completed = next;
        for expected_label in ["Continue", "Attack", "Continue", "Sleep"] {
            let event = completed.event.as_ref().expect("Spire Heart stage");
            assert_eq!(event.choices.len(), 1);
            assert_eq!(event.choices[0].label, expected_label);
            completed = crate::apply_event_action(
                &completed,
                crate::EventAction::Choose { choice_index: 0 },
            )
            .expect("Spire Heart choice advances");
        }
        assert_eq!(completed.phase, RunPhase::Complete);
        let terminal_event = completed
            .event
            .as_ref()
            .expect("terminal Spire Heart state");
        assert_eq!(terminal_event.event, Event::SpireHeart);
        assert_eq!(terminal_event.stage, 4);
        assert!(terminal_event.choices.is_empty());
        assert!(crate::legal_event_actions(&completed).is_empty());

        let json = serde_json::to_string(&completed).expect("serialize completed run state");
        let restored: RunState = serde_json::from_str(&json).expect("restore completed run state");
        assert_eq!(restored, completed);
    }

    #[test]
    fn neow_three_potions_hidden_reward_consumption_is_seed_dependent() {
        let mut duplicate_reroll_run =
            RunState::placeholder_seeded_ironclad(2_080_939_458_480_311_800_u64, 0);
        consume_neow_three_potions_hidden_card_reward(&mut duplicate_reroll_run);
        assert_eq!(duplicate_reroll_run.card_rng_counter, 10);
        assert_eq!(duplicate_reroll_run.card_rarity_factor, 5);
        duplicate_reroll_run.current_room_override = Some(RoomKind::Combat);
        enter_normal_combat_reward_screen(&mut duplicate_reroll_run);
        assert_eq!(
            reward_choice_ids(&duplicate_reroll_run),
            vec![POWER_THROUGH_ID, POMMEL_STRIKE_ID, WARCRY_ID]
        );

        let mut no_reroll_run = RunState::placeholder_seeded_ironclad(22_079_335_079, 0);
        consume_neow_three_potions_hidden_card_reward(&mut no_reroll_run);
        assert_eq!(no_reroll_run.card_rng_counter, 9);
        assert_eq!(no_reroll_run.card_rarity_factor, 2);
        no_reroll_run.current_room_override = Some(RoomKind::Combat);
        enter_normal_combat_reward_screen(&mut no_reroll_run);
        assert_eq!(
            reward_choice_ids(&no_reroll_run),
            vec![DUAL_WIELD_ID, WHIRLWIND_ID, HEAVY_BLADE_ID]
        );
    }

    #[test]
    fn test_seed_colorless_neow_carries_card_rng_through_first_two_combat_rewards() {
        let numeric_seed = 1_218_623_i64;
        let neow_reward =
            generate_neow_colorless_reward(numeric_seed, NeowRewardType::RandomColorless);

        let mut run = RunState::placeholder_seeded_ironclad(numeric_seed as u64, 0);
        run.card_rng_counter = neow_reward.card_rng_counter;
        run.gain_deck_card(SWIFT_STRIKE_ID);
        run.current_act = 1;
        run.current_room_override = Some(RoomKind::Combat);

        enter_normal_combat_reward_screen(&mut run);
        assert_eq!(
            reward_choice_ids(&run),
            vec![FIRE_BREATHING_ID, SPOT_WEAKNESS_ID, HEADBUTT_ID]
        );

        run.gain_deck_card(SPOT_WEAKNESS_ID);
        run.current_room_override = Some(RoomKind::Combat);
        enter_normal_combat_reward_screen(&mut run);
        assert_eq!(
            reward_choice_ids(&run),
            vec![THUNDERCLAP_ID, WARCRY_ID, METALLICIZE_ID]
        );
    }

    #[test]
    fn prayer_wheel_eagerly_consumes_both_hidden_reward_rolls_from_session_1224() {
        let mut run =
            RunState::placeholder_seeded_ironclad((-4_906_255_751_777_637_416_i64) as u64, 0);
        run.current_room_override = Some(RoomKind::Combat);
        run.card_rng_counter = 90;
        run.card_rarity_factor = -1;
        run.relics.push(Relic::QuestionCard);
        run.relics.push(Relic::PrayerWheel);

        enter_normal_combat_reward_screen(&mut run);

        let reward = run.reward.as_ref().expect("combat reward");
        assert_eq!(reward.pending_card_reward_count(), 2);
        assert!(reward.choices.is_empty());
        assert_eq!(reward.queued_card_rewards.len(), 2);
        assert!(reward
            .queued_card_rewards
            .iter()
            .all(|choices| choices.len() == 4));
        assert_eq!(run.card_rng_counter, 115);

        let skipped = apply_run_action(&run, RunAction::SkipReward)
            .expect("both unopened card rewards can be skipped");
        assert_eq!(skipped.card_rng_counter, 115);
        assert!(skipped.reward.is_none());
    }

    #[test]
    fn close_card_reward_preserves_choices_for_reopen() {
        let mut run = RunState::placeholder_seeded_ironclad(1_260_350_191_924, 0);
        run.current_room_override = Some(RoomKind::Combat);
        enter_normal_combat_reward_screen(&mut run);

        let opened = apply_run_action(&run, RunAction::OpenCardReward).expect("card reward opens");
        let original = reward_choice_ids(&opened);
        let opened_card_rng_counter = opened.card_rng_counter;
        assert!(opened.reward.as_ref().expect("reward").card_reward_active);

        let closed =
            apply_run_action(&opened, RunAction::CloseCardReward).expect("card reward closes");
        assert!(!closed.reward.as_ref().expect("reward").card_reward_active);
        assert_eq!(reward_choice_ids(&closed), original);
        assert_eq!(closed.card_rng_counter, opened_card_rng_counter);

        let reopened =
            apply_run_action(&closed, RunAction::OpenCardReward).expect("card reward reopens");
        assert!(reopened.reward.as_ref().expect("reward").card_reward_active);
        assert_eq!(reward_choice_ids(&reopened), original);
        assert_eq!(reopened.card_rng_counter, opened_card_rng_counter);
    }

    #[test]
    fn test_seed_scrap_ooze_then_big_fish_event_relics() {
        let numeric_seed = 1_218_623_i64;
        let mut run = RunState::map_fixture();
        run.relic_rng_seed = numeric_seed as u64;
        run.relics = vec![Relic::BurningBlood];
        run.current_act = 1;

        run.current_floor = 3;
        let act = run.current_act;
        let scrap_ooze_relic = roll_event_relic_reward(&mut run, act);
        assert_eq!(scrap_ooze_relic, RelicKey::DreamCatcher);
        run.gain_relic_key(scrap_ooze_relic);

        run.current_floor = 4;
        let act = run.current_act;
        let event_relic = roll_event_relic_reward(&mut run, act);
        assert_eq!(event_relic, RelicKey::ToxicEgg);
    }

    #[test]
    fn pen_nib_counter_persists_between_run_and_combat() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![Relic::PenNib];
        run.pen_nib_attacks_played = 9;

        let mut combat = CombatState::initial_fixture();
        combat.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        combat.relics = run.relics.clone();
        combat.relic_counters.pen_nib_attacks_played = run.pen_nib_attacks_played;
        run.combat = Some(combat);

        let next = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("strike plays");

        assert_eq!(next.pen_nib_attacks_played, 0);
        assert_eq!(
            next.combat
                .as_ref()
                .expect("combat remains active")
                .relic_counters
                .pen_nib_attacks_played,
            0
        );
    }

    #[test]
    fn ink_bottle_counter_persists_between_run_and_combat() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![Relic::InkBottle];
        run.ink_bottle_cards_played = 9;

        let mut combat = CombatState::initial_fixture();
        combat.piles.hand = vec![CardInstance::new(CardId::new(1), STRIKE_R_ID)];
        combat.piles.draw_pile = vec![CardInstance::new(CardId::new(2), SHOCKWAVE_PLUS_ID)];
        combat.relics = run.relics.clone();
        combat.relic_counters.ink_bottle_cards_played = run.ink_bottle_cards_played;
        run.combat = Some(combat);

        let next = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: CardId::new(1),
                target: Some(MonsterId::new(1)),
            },
        )
        .expect("strike plays");

        let combat = next.combat.as_ref().expect("combat remains active");
        assert_eq!(next.ink_bottle_cards_played, 0);
        assert_eq!(combat.relic_counters.ink_bottle_cards_played, 0);
        assert_eq!(
            combat.piles.hand.last().map(|card| card.content_id),
            Some(SHOCKWAVE_PLUS_ID)
        );
    }

    #[test]
    fn happy_flower_counter_persists_between_run_and_combat_turns() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![Relic::HappyFlower];
        run.happy_flower_turns = 1;

        let combat = run.init_combat(CombatState::initial_fixture());
        run.happy_flower_turns = combat.relic_counters.happy_flower_turns;
        run.combat = Some(combat);

        let next = apply_combat_action_on_run(&run, CombatAction::EndTurn).expect("turn ends");
        let combat = next.combat.as_ref().expect("combat remains active");

        assert_eq!(next.happy_flower_turns, 0);
        assert_eq!(combat.relic_counters.happy_flower_turns, 0);
        assert_eq!(combat.player.energy, 4);
    }

    #[test]
    fn sundial_counter_persists_between_combats_and_grants_third_shuffle_energy() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![Relic::Sundial];
        run.sundial_shuffles = 2;

        let mut base = CombatState::initial_fixture();
        base.piles.draw_pile.clear();
        base.monsters[0].intent = crate::MonsterIntent::Stun;
        let combat = run.init_combat(base);
        assert_eq!(combat.relic_counters.sundial_shuffles, 2);
        run.combat = Some(combat);

        let next = apply_combat_action_on_run(&run, CombatAction::EndTurn).expect("turn ends");
        let combat = next.combat.as_ref().expect("combat remains active");

        assert_eq!(next.sundial_shuffles, 3);
        assert_eq!(combat.relic_counters.sundial_shuffles, 3);
        assert_eq!(combat.player.energy, 5);
    }

    #[test]
    fn writhing_mass_mega_debuff_adds_parasite_and_triggers_ceramic_fish() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![Relic::CeramicFish];
        let starting_gold = run.gold;
        let starting_deck_len = run.deck.len();

        let mut combat = CombatState::initial_fixture();
        let mut writhing_mass = monster_state(&WRITHING_MASS_A0, MonsterId::new(1));
        writhing_mass.intent = crate::MonsterIntent::ApplyPlayerFrailAndWeak { frail: 2, weak: 2 };
        writhing_mass.move_history = vec![4];
        combat.monsters = vec![writhing_mass];
        run.combat = Some(combat);

        let next = apply_combat_action_on_run(&run, CombatAction::EndTurn)
            .expect("Writhing Mass Mega Debuff resolves");

        assert_eq!(next.deck.len(), starting_deck_len + 1);
        assert_eq!(
            next.deck.last().map(|card| card.content_id),
            Some(PARASITE_ID)
        );
        assert_eq!(next.gold, starting_gold + crate::relic::CERAMIC_FISH_GOLD);
        assert!(next.combat.as_ref().expect("combat continues").monsters[0].has_siphoned);
        let player_powers = &next
            .combat
            .as_ref()
            .expect("combat continues")
            .player
            .powers;
        assert_eq!(player_powers.frail, 0);
        assert_eq!(player_powers.weak, 0);
    }

    #[test]
    fn half_dead_darklings_still_allow_combat_gold_reward() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);

        let mut combat = CombatState::initial_fixture();
        for monster in &mut combat.monsters {
            monster.content_id = DARKLING_ID;
            monster.hp = 0;
            monster.alive = false;
            monster.escaped = true;
        }
        run.combat = Some(combat);

        enter_normal_combat_reward_screen(&mut run);

        assert!(
            run.reward.as_ref().expect("reward screen").gold_offer > 0,
            "Darkling half-dead markers are not escaped-monster gold suppression"
        );
    }

    #[test]
    fn faded_transient_still_allows_normal_combat_rewards() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.potion_rng_seed = 772_776_727_775;
        run.potion_rng_counter = 88;

        let mut combat = CombatState::initial_fixture();
        for monster in &mut combat.monsters {
            monster.content_id = TRANSIENT_ID;
            monster.hp = 0;
            monster.alive = false;
            monster.escaped = true;
        }
        run.combat = Some(combat);

        enter_normal_combat_reward_screen(&mut run);

        let reward = run.reward.as_ref().expect("reward screen");
        assert!(reward.gold_offer > 0);
        assert_eq!(reward.potion_offer, Some(Potion::Elixir));
        assert_eq!(run.potion_rng_counter, 91);
        assert_eq!(run.potion_chance, -10);
    }

    #[test]
    fn boss_relic_chest_cannot_be_opened_after_boss_relic_pick() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(RoomKind::Boss);
        run.current_floor = 17;
        run.current_act = 1;
        run.relic_rng_seed = 1_218_623;
        run.relics = vec![Relic::BurningBlood];

        assert!(validate_treasure_action(&run, RunAction::OpenChest).is_ok());
        assert!(validate_treasure_action(&run, RunAction::Proceed).is_err());

        let opened = apply_run_action(&run, RunAction::OpenChest).expect("boss chest opens");
        assert!(opened.boss_chest_opened);
        assert_eq!(opened.phase, RunPhase::Reward);
        assert!(!opened
            .reward
            .as_ref()
            .expect("boss relic reward")
            .boss_relic_choices
            .is_empty());

        let original_choices = opened
            .reward
            .as_ref()
            .expect("boss relic reward")
            .boss_relic_choices
            .clone();
        let boss_pool_after_open = opened
            .relic_pools
            .as_ref()
            .expect("relic pools initialized")
            .boss
            .clone();
        let skipped = apply_run_action(&opened, RunAction::SkipReward)
            .expect("boss relic reward can be closed");
        assert_eq!(skipped.phase, RunPhase::Treasure);
        assert!(!skipped.boss_chest_opened);
        assert!(skipped.reward.is_none());
        assert_eq!(skipped.pending_boss_relic_choices, original_choices);

        let reopened = apply_run_action(&skipped, RunAction::OpenChest)
            .expect("closed boss relic reward can be reopened");
        assert_eq!(
            reopened
                .reward
                .as_ref()
                .expect("reopened boss relic reward")
                .boss_relic_choices,
            original_choices
        );
        assert_eq!(
            reopened
                .relic_pools
                .as_ref()
                .expect("relic pools remain initialized")
                .boss,
            boss_pool_after_open
        );

        let picked = apply_run_action(&reopened, RunAction::ChooseBossRelicReward { index: 0 })
            .expect("boss relic can be picked");
        assert!(picked.boss_chest_opened);
        assert_eq!(picked.phase, RunPhase::Treasure);
        assert!(picked.reward.is_none());
        assert!(picked.pending_boss_relic_choices.is_empty());
        assert!(validate_treasure_action(&picked, RunAction::OpenChest).is_err());
        assert!(validate_treasure_action(&picked, RunAction::Proceed).is_ok());
    }

    #[test]
    fn mark_of_bloom_blocks_the_act_transition_heal() {
        let mut run = RunState::map_fixture();
        run.current_act = 1;
        run.player_hp = 10;
        run.player_max_hp = 80;
        run.relic_keys.push(RelicKey::MarkOfBloom);

        enter_next_act_map(&mut run);

        assert_eq!(run.current_act, 2);
        assert_eq!(run.player_hp, 10);
    }

    #[test]
    fn lizard_tail_revives_mid_monster_turn_and_remaining_sentries_act() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Elite);
        run.relics.push(Relic::LizardTail);

        let mut combat = CombatState::sentry_fixture();
        combat.player.hp = 1;
        combat.player.max_hp = 85;
        combat.monsters[0].intent = crate::MonsterIntent::Attack { damage: 9 };
        combat.monsters[1].intent = crate::MonsterIntent::AddDazedToDiscard { count: 2 };
        combat.monsters[2].intent = crate::MonsterIntent::Attack { damage: 9 };
        run.combat = Some(combat);

        let after = apply_combat_action_on_run(&run, CombatAction::EndTurn)
            .expect("enemy turn resolves through revival");
        let combat = after.combat.expect("combat continues");

        assert_eq!(combat.player.hp, 33);
        let dazed_count = combat
            .piles
            .hand
            .iter()
            .chain(&combat.piles.draw_pile)
            .chain(&combat.piles.discard_pile)
            .chain(&combat.piles.exhaust_pile)
            .filter(|card| card.content_id == crate::content::cards::DAZED_ID)
            .count();
        assert_eq!(dazed_count, 2);
        assert!(after.lizard_tail_used);
    }

    #[test]
    fn upgraded_starter_relic_keeps_starter_relic_slot() {
        let mut run = RunState::map_fixture();
        run.relics = vec![
            Relic::BurningBlood,
            Relic::CentennialPuzzle,
            Relic::OddlySmoothStone,
        ];

        run.gain_relic(Relic::BlackBlood);

        assert_eq!(
            run.relics,
            vec![
                Relic::BlackBlood,
                Relic::CentennialPuzzle,
                Relic::OddlySmoothStone
            ]
        );
    }

    #[test]
    fn guaranteed_potion_still_consumes_drop_roll() {
        let mut actual_rng = StsRng::new(77);
        let mut expected_rng = StsRng::new(77);
        let mut potion_chance = 0;

        let actual = target_potion_reward_offer(&mut actual_rng, &mut potion_chance, 2, 0, 3, true);
        let _drop_roll = expected_rng.random_int(99);
        let expected = Some(target_random_potion(&mut expected_rng));

        assert_eq!(actual, expected);
        assert_eq!(actual_rng.counter(), expected_rng.counter());
        assert_eq!(potion_chance, -10);
    }
}
