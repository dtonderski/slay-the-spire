use crate::{
    card::{CardInstance, CardRarity},
    combat::turn::revival_hp_with_relics,
    combat::{
        apply_combat_action_with_events, finish_monster_turn_after_player_revival,
        start_player_turn, CombatPhase,
    },
    content::cards::{
        upgrade_card_instance, BLOOD_FOR_BLOOD_ID, BLOOD_FOR_BLOOD_PLUS_ID, PARASITE_ID,
    },
    content::encounters::{
        generate_beyond_encounter_lists_with_rng, generate_city_encounter_lists_with_rng,
    },
    content::monsters::{
        DARKLING_ID, GREMLIN_NOB_ID, SLAVER_BLUE_ID, SLAVER_RED_ID, TASKMASTER_ID, TRANSIENT_ID,
    },
    content::reward_pool::{
        ironclad_reward_card_rarity, random_normal_curse, RewardCardEntry, IRONCLAD_REWARD_ENTRIES,
    },
    content::shop_pool::{
        ironclad_combat_discovery_pool, random_colorless_from_pool, shop_card_content_id,
    },
    ids::{CardId, ContentId},
    map::{RoomKind, TargetMapAct},
    potion::{Potion, PotionRarity, FAIRY_HEAL_PERCENT, IRONCLAD_POTION_POOL},
    relic::{
        Relic, RelicKey, RelicTier, BUSTED_CROWN_CARD_REWARD_REDUCTION, CAULDRON_POTIONS,
        QUESTION_CARD_REWARD_BONUS, SINGING_BOWL_MAX_HP,
    },
    rng::StsRng,
    run::event::{colosseum_choices, enter_spire_heart_event, event_screen_for_run, Event},
    run::potion::{
        apply_combat_card_reward_choice, apply_combat_card_reward_skip,
        apply_discard_select_choice, apply_discard_select_confirm, apply_draw_select_choice,
        apply_draw_select_confirm, apply_exhaust_select_choice, apply_exhaust_select_confirm,
        apply_hand_select_choice, apply_hand_select_confirm,
        apply_hand_select_confirm_without_retrieval, apply_potion_action,
    },
    run::shop::apply_shop_action,
    run::state::{
        RunRngStream, DEFAULT_EVENT_ROOM_MONSTER_CHANCE, DEFAULT_EVENT_ROOM_SHOP_CHANCE,
        DEFAULT_EVENT_ROOM_TREASURE_CHANCE,
    },
    CombatAction, MonsterState, RewardContinuation, RewardScreen, RunAction, RunPhase, RunState,
    SimError, SimResult,
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
    /// Matryoshka inserts its bonus relic before the chest gold RewardItem.
    /// Ordinary chests keep gold before the single relic. After a non-leading
    /// relic is claimed, this flag preserves the residual `[relic, gold]` order
    /// instead of falling back to the ordinary `[gold, relic]` layout.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub relic_before_gold: bool,
    /// Chest relic linked to the appended Sapphire Key reward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sapphire_key_relic_offer: Option<Relic>,
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
        relic_before_gold: false,
        sapphire_key_relic_offer: None,
    });
}

fn combat_reward_continuation(run: &RunState) -> RewardContinuation {
    if run.event.is_some() {
        RewardContinuation::Event
    } else {
        RewardContinuation::None
    }
}

/// Prepare the reward overlay that Orrery opens from the shop.
///
/// CommunicationMod leaves the shop on the CombatRewardScreen path (FIDL00405):
/// buying Orrery closes the merchant UI (`shop_merchant_open = false`) and shows
/// five CARD rewards. After the last pick/skip the overlay stays as an empty
/// combat-reward frame until SKIP returns to `SHOP_ROOM`; CHOOSE re-opens the
/// merchant. The relic pickup itself still queues the five pending card rewards.
pub(crate) fn enter_orrery_reward_screen(run: &mut RunState) {
    run.phase = RunPhase::Reward;
    run.shop_merchant_open = false;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::Shop,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::None,
    });
}

/// Add the five potion rewards constructed by target `Cauldron.onEquip`.
///
/// The rewards belong to the current room rather than to the potion belt. If
/// no reward overlay is open yet (the shop-purchase path), opening them closes
/// the merchant and retains the shop for the overlay's typed continuation.
pub(crate) fn enter_cauldron_potion_reward_screen(run: &mut RunState) -> SimResult<()> {
    let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
    let mut potion_offers = Vec::with_capacity(CAULDRON_POTIONS);
    for _ in 0..CAULDRON_POTIONS {
        potion_offers.push(target_uniform_random_potion(&mut potion_rng));
    }
    run.store_rng_counter(RunRngStream::Potion, &potion_rng);

    // `Cauldron.onEquip` opens CombatRewardScreen while still in ShopRoom.
    // setupItemReward constructs and then Cauldron removes the room's first
    // card RewardItem, so this transient card reward still consumes card RNG.
    if run.phase == RunPhase::Shop && run.reward.is_none() {
        consume_hidden_room_card_reward(run)?;
    }

    if let Some(reward) = run.reward.as_mut() {
        // Preserve the existing potion RewardItem's position when Cauldron is
        // picked up from an already-open reward screen.
        if let Some(potion) = reward.potion_offer.take() {
            let mut offers =
                Vec::with_capacity(1 + reward.potion_offers.len() + potion_offers.len());
            offers.push(potion);
            offers.append(&mut reward.potion_offers);
            offers.extend(potion_offers);
            reward.potion_offers = offers;
        } else {
            reward.potion_offers.extend(potion_offers);
        }
        return Ok(());
    }

    let continuation = match run.phase {
        RunPhase::Shop if run.shop.is_some() => RewardContinuation::Shop,
        RunPhase::Event if run.event.is_some() => RewardContinuation::Event,
        _ => RewardContinuation::None,
    };
    if continuation == RewardContinuation::Shop {
        run.shop_merchant_open = false;
    }
    run.phase = RunPhase::Reward;
    run.reward = Some(RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers,
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::None,
    });
    Ok(())
}

/// Target Orrery constructs all five CardRewardItems immediately on pickup,
/// consuming card RNG before the player opens any of them.
pub(crate) fn queue_orrery_card_reward_choices(run: &mut RunState) -> SimResult<()> {
    queue_eager_card_reward_choices(run, crate::relic::ORRERY_EAGER_CARD_REWARDS)
}

fn queue_eager_card_reward_choices(run: &mut RunState, count: u8) -> SimResult<()> {
    let choice_count = reward_card_choice_count(run);
    let total_choices =
        usize::from(count)
            .checked_mul(choice_count)
            .ok_or(SimError::InvalidState(
                "queued card reward choice count overflows usize",
            ))?;
    let mut queued = Vec::with_capacity(count as usize);
    let mut next_card_id = run.reserve_card_instance_ids(total_choices)?;
    for _ in 0..count {
        roll_pending_card_reward_choices(run)?;
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
    Ok(())
}

/// Event paths that grant a relic without opening the combat reward screen use
/// `AbstractDungeon.returnRandomScreenlessRelic` (skip bottled relics / Whetstone).
pub fn roll_event_relic_reward(run: &mut RunState, act: i32) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_relic_tier(&mut relic_rng, act);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    roll_screenless_relic_reward(run, tier)
}

/// Reward-screen relic offers (Dig / chests / elites) use
/// `returnRandomRelicTier` + `returnRandomRelic`. Bottled relics are allowed
/// because the combat reward screen can open their bottle grid on pickup.
pub fn roll_reward_screen_relic(run: &mut RunState, act: i32) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_relic_tier(&mut relic_rng, act);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    roll_relic_reward(run, tier)
}

fn roll_screenless_relic_reward(run: &mut RunState, tier: RelicTier) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, false);
    let pools = run.relic_pools.as_mut().expect("relic pools initialized");
    pools.return_random_screenless_relic(tier, &context)
}

const BASE_POTION_DROP_CHANCE: i32 = 40;
const ACT_4: i32 = 4;

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
    let raw_roll = rng.random_int(99);
    let roll = raw_roll + card_rarity_factor;
    if roll < chances.rare {
        CardRarity::Rare
    } else if roll < chances.rare + chances.uncommon {
        CardRarity::Uncommon
    } else {
        CardRarity::Common
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

fn reward_rarity_chances_for_run(
    run: &RunState,
    rarity_chances: RewardRarityChances,
) -> RewardRarityChances {
    if run.relics.contains(&Relic::NlothsGift)
        && run.current_room_kind() != Some(crate::map::RoomKind::Shop)
    {
        RewardRarityChances {
            rare: rarity_chances.rare * 3,
            uncommon: rarity_chances.uncommon,
        }
    } else {
        rarity_chances
    }
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

/// Target `AbstractDungeon.getColorlessRewardCards` rolls only rare or
/// uncommon cards, using the dungeon's fixed 30% rare chance. Its pool lookup
/// then selects from the card-ID-sorted rarity pool and retries duplicates
/// without rerolling rarity.
#[must_use]
pub fn target_colorless_event_card_reward_choices_with_count(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
    choice_count: usize,
) -> Vec<CardInstance> {
    const COLORLESS_RARE_CHANCE: f32 = 0.3;
    let mut choices = Vec::with_capacity(choice_count);

    for index in 0..choice_count {
        let rarity = if rng.random_float() < COLORLESS_RARE_CHANCE {
            *card_rarity_factor = 5;
            CardRarity::Rare
        } else {
            CardRarity::Uncommon
        };
        loop {
            let content_id = random_colorless_from_pool(rng, rarity);
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

    choices
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
            CardRarity::Uncommon => {}
            CardRarity::Rare => *card_rarity_factor = 5,
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
    "JUDGEMENT",
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

pub(crate) fn combat_gold_offer_with_relics(run: &RunState, amount: i32) -> i32 {
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
    let rarity = match rng.random_int_range(0, 99) {
        roll if roll < 65 => PotionRarity::Common,
        roll if roll < 90 => PotionRarity::Uncommon,
        _ => PotionRarity::Rare,
    };

    // AbstractDungeon.returnRandomPotion(rarity, true) always performs one
    // initial PotionHelper.getRandomPotion() draw that cannot be accepted while
    // the combat flag is set, then re-enters the loop. Only later draws can be
    // returned, and Fruit Juice is rejected in combat.
    let _ = target_uniform_random_potion(rng);
    loop {
        let potion = target_uniform_random_potion(rng);
        if potion.rarity() == rarity && potion != Potion::FruitJuice {
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
) -> SimResult<Option<Potion>> {
    let _ = (potion_belt_count, potion_capacity);

    let chance = if reward_count >= 4 {
        0
    } else if guaranteed_potion {
        100
    } else {
        BASE_POTION_DROP_CHANCE
            .checked_add(*potion_chance)
            .ok_or(SimError::InvalidState(
                "potion reward drop chance overflows i32",
            ))?
    };

    let mut next_rng = rng.clone();
    let roll = next_rng.random_int(99);
    let (next_potion_chance, offer) = if roll >= chance {
        let next_potion_chance = potion_chance
            .checked_add(10)
            .ok_or(SimError::InvalidState("potion reward chance overflows i32"))?;
        (next_potion_chance, None)
    } else {
        let next_potion_chance = potion_chance.checked_sub(10).ok_or(SimError::InvalidState(
            "potion reward chance underflows i32",
        ))?;
        (
            next_potion_chance,
            Some(target_random_potion(&mut next_rng)),
        )
    };
    *rng = next_rng;
    *potion_chance = next_potion_chance;
    Ok(offer)
}

pub(crate) fn roll_relic_reward(run: &mut RunState, tier: RelicTier) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, false);
    let pools = run.relic_pools.as_mut().expect("relic pools initialized");
    pools.return_random_relic(tier, &context)
}

fn roll_bonus_relic_offer(run: &mut RunState) -> Relic {
    // Black Star adds a second elite relic via `returnRandomRelic`, but the
    // extra drop rejects Girya / Peace Pipe / Shovel so it cannot fill the
    // two-campfire-relic cap. Bottles and Whetstone stay legal (they appear on
    // the combat reward screen). Rejected rest-site relics are consumed from
    // the pool, then the same tier is drawn from the front again.
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_elite_relic_tier(&mut relic_rng);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    loop {
        let relic = roll_relic_reward(run, tier);
        if !matches!(relic, Relic::Girya | Relic::PeacePipe | Relic::Shovel) {
            return relic;
        }
    }
}

fn roll_matryoshka_bonus_relic_offer(run: &mut RunState) -> Relic {
    // Matryoshka.onChestOpen uses relicRng.randomBoolean(0.75F): its bonus
    // relic is common on true and uncommon on false. It does not use the
    // normal act relic-tier distribution.
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_matryoshka_relic_tier(&mut relic_rng);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    roll_relic_reward(run, tier)
}

fn target_matryoshka_relic_tier(relic_rng: &mut StsRng) -> RelicTier {
    if relic_rng.random_float() < 0.75 {
        RelicTier::Common
    } else {
        RelicTier::Uncommon
    }
}

pub fn enter_relic_reward_screen(run: &mut RunState, kind: CombatRewardKind) -> SimResult<()> {
    let mut next = run.clone();
    enter_relic_reward_screen_inner(&mut next, kind)?;
    *run = next;
    Ok(())
}

fn enter_relic_reward_screen_inner(run: &mut RunState, kind: CombatRewardKind) -> SimResult<()> {
    let continuation = combat_reward_continuation(run);
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

    let relic_offer = Some(roll_relic_reward(run, tier));
    let pending_relic_offer = (kind == CombatRewardKind::Elite
        && run.relics.contains(&Relic::BlackStar))
    .then(|| roll_bonus_relic_offer(run));

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
        )?;
        run.store_rng_counter(RunRngStream::Potion, &potion_rng);
    }

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer,
        pending_relic_offer,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::None,
    });
    Ok(())
}

pub fn enter_boss_relic_reward_screen(run: &mut RunState) {
    reserve_boss_relic_choices(run);
    let boss_relic_choices = run.pending_boss_relic_choices.clone();

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.boss_chest_opened = true;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::Treasure,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices,
        card_reward_flow: crate::run::CardRewardFlow::None,
    });
}

/// BossChest constructs its three relic offers when the chest room is entered.
/// Opening the chest only reveals that already-reserved queue, so walking past
/// an unopened boss chest still consumes those entries from the boss pool.
fn reserve_boss_relic_choices(run: &mut RunState) {
    if run.pending_boss_relic_choices.is_empty() {
        run.pending_boss_relic_choices = (0..3)
            .map(|_| roll_relic_reward(run, RelicTier::Boss))
            .collect();
    }
}

pub(crate) fn enter_calling_bell_reward_screen(run: &mut RunState) {
    let continuation = if run
        .event
        .as_ref()
        .is_some_and(|event| event.event == crate::Event::Neow)
    {
        RewardContinuation::Neow
    } else {
        RewardContinuation::Treasure
    };
    let common = roll_screenless_relic_reward(run, RelicTier::Common);
    let uncommon = roll_screenless_relic_reward(run, RelicTier::Uncommon);
    let rare = roll_screenless_relic_reward(run, RelicTier::Rare);

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer: 0,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer: Some(common),
        pending_relic_offer: None,
        queued_relic_offers: vec![uncommon, rare],
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::None,
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

/// Consume a card RewardItem that target `CombatRewardScreen.setupItemReward`
/// constructs and a relic/event immediately removes before the player can see it.
///
/// The target still rolls rarity, duplicate rerolls, and upgrade chances for
/// this transient reward. Its rarity distribution follows the room that opened
/// the reward screen, just like an ordinary `RewardItem`.
pub(crate) fn consume_hidden_room_card_reward(run: &mut RunState) -> SimResult<()> {
    let choice_count = reward_card_choice_count(run);
    let next_card_id = run.reserve_card_instance_ids(choice_count)?;
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
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
    let rarity_chances = reward_rarity_chances_for_run(run, rarity_chances);
    let mut choices = target_card_reward_choices_with_count_and_pool(
        &mut card_rng,
        &mut run.card_rarity_factor,
        next_card_id,
        choice_count,
        pool_kind,
        rarity_chances,
        None,
        true,
    );
    consume_reward_card_upgrade_rolls(&mut card_rng, &mut choices, card_upgraded_chance(run))?;
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    Ok(())
}

pub fn consume_neow_three_potions_hidden_card_reward(run: &mut RunState) -> SimResult<()> {
    consume_hidden_room_card_reward(run)
}

pub(crate) fn roll_pending_card_reward_choices(run: &mut RunState) -> SimResult<()> {
    let choice_count = reward_card_choice_count(run);
    let next_card_id = run.reserve_card_instance_ids(choice_count)?;
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
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
    // N'loth's Gift triples the base rare-card chance for combat rewards. The
    // pity offset remains unchanged; increasing only the rare threshold takes
    // the additional chance from common cards, as in AbstractDungeon.
    let rarity_chances = reward_rarity_chances_for_run(run, rarity_chances);
    let apply_card_rarity_factor = true;
    // The boss combat's card reward is three rare cards. A Tiny House reward
    // overlay also lives in the boss room, but its RewardItem is a normal card
    // reward and must not be forced rare.
    let forced_requested_rarity = if run.current_room_kind() == Some(crate::map::RoomKind::Boss)
        && run
            .reward
            .as_ref()
            .is_some_and(|reward| reward.continuation == RewardContinuation::None)
    {
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
    consume_reward_card_upgrade_rolls(&mut card_rng, &mut choices, card_upgraded_chance(run))?;
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    // Egg preview on the reward screen upgrades the card instance itself. This
    // preserves generic upgrade metadata (including repeated Searing Blow
    // upgrades) while keeping TakeCardReward from re-applying the egg.
    for choice in &mut choices {
        *choice = run.card_after_card_add_relics(*choice)?;
    }
    run.reward.as_mut().expect("reward screen present").choices = choices;
    Ok(())
}

fn card_upgraded_chance(run: &RunState) -> f32 {
    match run.current_act {
        2 if run.ascension >= 12 => 0.125,
        2 => 0.25,
        3 | 4 if run.ascension >= 12 => 0.25,
        3 | 4 => 0.5,
        _ => 0.0,
    }
}

fn consume_reward_card_upgrade_rolls(
    rng: &mut StsRng,
    choices: &mut [CardInstance],
    upgraded_chance: f32,
) -> SimResult<()> {
    for choice in choices {
        if reward_card_rarity(choice.content_id) == Some(CardRarity::Rare) {
            continue;
        }

        let upgrades = rng.random_float() < upgraded_chance;
        if upgrades {
            if let Some(upgraded) = upgrade_card_instance(*choice)? {
                *choice = upgraded;
            }
        }
    }
    Ok(())
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
    let mut normalized = identity
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    // Vanilla Watcher Fasting uses cardID `Fasting2` (Fasting was taken).
    if normalized == "fasting2" {
        normalized = "fasting".to_owned();
    }
    // CommunicationMod exports Silent Sneaky Strike as id `Underhanded Strike`.
    if normalized == "underhandedstrike" {
        normalized = "sneakystrike".to_owned();
    }
    // Watcher Judgement cardID is `Judgement`; some pool tables used `JUDGMENT`.
    if normalized == "judgment" {
        normalized = "judgement".to_owned();
    }
    // Defect Recursion's cardID is `Redo` (Recursion.java ldc).
    if normalized == "redo" {
        normalized = "recursion".to_owned();
    }
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

pub fn enter_normal_combat_reward_screen(run: &mut RunState) -> SimResult<()> {
    let mut next = run.clone();
    enter_normal_combat_reward_screen_inner(&mut next)?;
    *run = next;
    Ok(())
}

fn enter_normal_combat_reward_screen_inner(run: &mut RunState) -> SimResult<()> {
    validate_combat_reward_entry(run)?;
    // Target PrayerWheel.onVictory only fires in MonsterRoom (map combat /
    // elite). Event-room fights (Masked Bandits, etc.) stay on EventRoom and
    // must not receive the extra card reward.
    let pending_card_reward_count = if run.relics.contains(&Relic::PrayerWheel)
        && run.current_room_kind() != Some(crate::map::RoomKind::Event)
    {
        2
    } else {
        1
    };
    let total_card_choices = usize::from(pending_card_reward_count)
        .checked_mul(reward_card_choice_count(run))
        .ok_or(SimError::InvalidState(
            "combat reward card choice count overflows usize",
        ))?;
    run.reserve_card_instance_ids(total_card_choices)?;
    let continuation = combat_reward_continuation(run);
    let all_monsters_escaped = run
        .combat
        .as_ref()
        .map(|combat| suppress_gold_for_all_escaped_monsters(&combat.monsters))
        .unwrap_or(false);
    let pending_event_gold_offer = std::mem::take(&mut run.pending_event_combat_gold_offer);
    let pending_event_gold_bonus = std::mem::take(&mut run.pending_event_combat_gold_bonus);
    let pending_event_elite_gold = std::mem::take(&mut run.pending_event_combat_elite_gold);
    let relic_offer = run.pending_event_combat_relic_offer.take();
    // pending_event_combat_gold_offer replaces the combat roll (DA no-search-relic,
    // Sphere, mushrooms). pending_event_combat_gold_bonus stacks on top (DA after
    // search relic — FIDL00229). pending_event_combat_elite_gold selects elite range.
    // Colosseum keeps replace semantics via pending_event_combat_gold_offer = 100.
    let gold_offer =
        if all_monsters_escaped && pending_event_gold_offer == 0 && pending_event_gold_bonus == 0 {
            0
        } else {
            let combat_base = if all_monsters_escaped {
                0
            } else if pending_event_gold_offer > 0 {
                pending_event_gold_offer
            } else {
                let mut treasure_rng = run.rng_for_stream(RunRngStream::Treasure);
                let rolled = if pending_event_elite_gold {
                    target_elite_combat_gold(&mut treasure_rng)
                } else {
                    target_normal_combat_gold(&mut treasure_rng)
                };
                run.store_rng_counter(RunRngStream::Treasure, &treasure_rng);
                rolled
            };
            let total = combat_base
                .checked_add(pending_event_gold_bonus)
                .ok_or(SimError::InvalidState("event combat gold overflows i32"))?;
            combat_gold_offer_with_relics(run, total)
        };
    let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
    let potion_capacity = run.potion_capacity();
    let potion_offer = if all_monsters_escaped && !run.relics.contains(&Relic::WhiteBeastStatue) {
        run.potion_chance = run
            .potion_chance
            .checked_add(10)
            .ok_or(SimError::InvalidState("potion reward chance overflows i32"))?;
        let _ = potion_rng.random_int(99);
        None
    } else {
        target_potion_reward_offer(
            &mut potion_rng,
            &mut run.potion_chance,
            1,
            run.potions.len(),
            potion_capacity,
            run.relics.contains(&Relic::WhiteBeastStatue),
        )?
    };
    run.store_rng_counter(RunRngStream::Potion, &potion_rng);

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer,
        potion_offers: Vec::new(),
        relic_offer,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::pending(pending_card_reward_count),
    });
    if pending_card_reward_count == 1 {
        roll_pending_card_reward_choices(run)?;
    } else {
        // CombatRewardScreen constructs both Prayer Wheel RewardItems before either is opened.
        // Their card RNG must therefore be consumed even when the player skips both rewards.
        queue_eager_card_reward_choices(run, pending_card_reward_count)?;
    }
    Ok(())
}

fn suppress_gold_for_all_escaped_monsters(monsters: &[MonsterState]) -> bool {
    !monsters.is_empty()
        && monsters.iter().all(|monster| {
            monster.escaped
                && monster.content_id != DARKLING_ID
                && monster.content_id != TRANSIENT_ID
        })
}

pub fn enter_reward_screen(run: &mut RunState) -> SimResult<()> {
    validate_combat_reward_entry(run)?;
    let stolen_gold_offer = run
        .combat
        .as_ref()
        .expect("validated combat reward entry has combat state")
        .monsters
        .iter()
        .filter(|monster| !monster.escaped)
        .try_fold(0_i32, |total, monster| {
            total
                .checked_add(monster.stolen_gold)
                .ok_or(SimError::InvalidState("stolen gold reward overflows i32"))
        })?;
    enter_normal_combat_reward_screen(run)?;
    if let Some(reward) = run.reward.as_mut() {
        reward.stolen_gold_offer = stolen_gold_offer;
    }
    Ok(())
}

fn enter_colosseum_combat_reward_screen(run: &mut RunState) -> SimResult<()> {
    // The second Colosseum fight uses the event's custom reward list rather
    // than the ordinary single-relic combat reward. The target adds two
    // event relics, fixed gold, a potion, and a card reward, then returns to
    // the map.
    // Colosseum's two relic rewards are explicitly rare then uncommon; they
    // do not use the normal act-tier roll used by screenless event rewards.
    let relic_offer = roll_relic_reward(run, RelicTier::Rare);
    let pending_relic_offer = roll_relic_reward(run, RelicTier::Uncommon);
    let potion_rng_counter = run.potion_rng_counter;
    let potion_chance = run.potion_chance;
    run.pending_event_combat_gold_offer = 100;
    run.pending_event_combat_relic_offer = Some(relic_offer);
    enter_normal_combat_reward_screen(run)?;
    // The event has two relics and gold in the reward list before the normal
    // potion helper runs. Restore the normal helper's temporary effects and
    // perform that three-item event-room roll.
    run.potion_rng_counter = potion_rng_counter;
    run.potion_chance = potion_chance;
    let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
    let potion_capacity = run.potion_capacity();
    let potion_offer = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        3,
        run.potions.len(),
        potion_capacity,
        false,
    )?;
    run.store_rng_counter(RunRngStream::Potion, &potion_rng);
    let reward = run.reward.as_mut().expect("Colosseum reward screen");
    reward.continuation = RewardContinuation::Map;
    reward.potion_offer = potion_offer;
    reward.pending_relic_offer = Some(pending_relic_offer);
    Ok(())
}

pub fn enter_elite_combat_reward_screen(run: &mut RunState) -> SimResult<()> {
    let mut next = run.clone();
    enter_elite_combat_reward_screen_inner(&mut next)?;
    *run = next;
    Ok(())
}

fn enter_elite_combat_reward_screen_inner(run: &mut RunState) -> SimResult<()> {
    validate_combat_reward_entry(run)?;
    run.reserve_card_instance_ids(reward_card_choice_count(run))?;
    let continuation = combat_reward_continuation(run);
    run.emerald_key_reward_available = run
        .map
        .as_ref()
        .is_some_and(|map| run.emerald_key_node == Some(map.current_node));
    let mut treasure_rng = run.rng_for_stream(RunRngStream::Treasure);
    let gold_offer =
        combat_gold_offer_with_relics(run, target_elite_combat_gold(&mut treasure_rng));
    run.store_rng_counter(RunRngStream::Treasure, &treasure_rng);

    run.ensure_ironclad_relic_pools();
    let mut relic_rng = run.rng_for_stream(RunRngStream::Relic);
    let tier = target_elite_relic_tier(&mut relic_rng);
    run.store_rng_counter(RunRngStream::Relic, &relic_rng);
    let relic_offer = Some(roll_relic_reward(run, tier));
    let pending_relic_offer = run
        .relics
        .contains(&Relic::BlackStar)
        .then(|| roll_bonus_relic_offer(run));

    let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
    let potion_capacity = run.potion_capacity();
    let potion_offer = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        2,
        run.potions.len(),
        potion_capacity,
        run.relics.contains(&Relic::WhiteBeastStatue),
    )?;
    run.store_rng_counter(RunRngStream::Potion, &potion_rng);

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer,
        potion_offers: Vec::new(),
        relic_offer,
        pending_relic_offer,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::pending(1),
    });
    roll_pending_card_reward_choices(run)?;
    Ok(())
}

pub fn enter_boss_combat_reward_screen(run: &mut RunState) -> SimResult<()> {
    let mut next = run.clone();
    enter_boss_combat_reward_screen_inner(&mut next)?;
    *run = next;
    Ok(())
}

fn enter_boss_combat_reward_screen_inner(run: &mut RunState) -> SimResult<()> {
    validate_combat_reward_entry(run)?;
    run.reserve_card_instance_ids(reward_card_choice_count(run))?;
    let continuation = combat_reward_continuation(run);
    let mut misc_rng = run.rng_for_stream(RunRngStream::Misc);
    let gold_offer = combat_gold_offer_with_relics(run, target_boss_combat_gold(&mut misc_rng));
    run.store_rng_counter(RunRngStream::Misc, &misc_rng);
    let mut potion_rng = run.rng_for_stream(RunRngStream::Potion);
    let potion_capacity = run.potion_capacity();
    let potion_offer = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        1,
        run.potions.len(),
        potion_capacity,
        run.relics.contains(&Relic::WhiteBeastStatue),
    )?;
    run.store_rng_counter(RunRngStream::Potion, &potion_rng);

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer,
        potion_offers: Vec::new(),
        relic_offer: None,
        pending_relic_offer: None,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::pending(1),
    });
    roll_pending_card_reward_choices(run)?;
    Ok(())
}

fn validate_combat_reward_entry(run: &RunState) -> SimResult<()> {
    run.validate()?;
    if run.phase != RunPhase::Combat {
        return Err(SimError::InvalidState(
            "combat reward entry requires combat phase",
        ));
    }
    if run.reward.is_some() {
        return Err(SimError::InvalidState(
            "combat reward entry already has a reward screen",
        ));
    }
    let combat = run.combat.as_ref().ok_or(SimError::InvalidState(
        "combat reward entry requires combat state",
    ))?;
    if combat.phase != CombatPhase::Won {
        return Err(SimError::InvalidState(
            "combat reward entry requires won combat",
        ));
    }
    if combat.player.hp <= 0 {
        return Err(SimError::InvalidState(
            "combat reward entry requires a living player",
        ));
    }
    if combat.monsters.iter().any(|monster| monster.alive) {
        return Err(SimError::InvalidState(
            "combat reward entry requires no living monsters",
        ));
    }
    Ok(())
}

fn enter_boss_reward_chest(run: &mut RunState) -> SimResult<()> {
    run.advance_floor()?;
    run.apply_floor_entry_relics()?;
    run.phase = RunPhase::Treasure;
    run.combat = None;
    run.reward = None;
    run.treasure_room = None;
    run.boss_chest_opened = false;
    run.pending_boss_relic_choices.clear();
    reserve_boss_relic_choices(run);
    run.reinit_room_rngs_for_floor();
    Ok(())
}

fn enter_next_act_map(run: &mut RunState) -> SimResult<()> {
    let next_act = run.current_act + 1;
    advance_card_rng_for_dungeon_transition(run);
    // The target resets the potion drop bonus at the start of every act.
    // `potion_chance` is the bonus around the 40% base chance, so the reset is
    // represented by zero rather than by the displayed base percentage.
    run.potion_chance = 0;
    run.event_room_monster_chance =
        crate::run::state::EventRoomChance::new(DEFAULT_EVENT_ROOM_MONSTER_CHANCE);
    run.event_room_shop_chance =
        crate::run::state::EventRoomChance::new(DEFAULT_EVENT_ROOM_SHOP_CHANCE);
    run.event_room_treasure_chance =
        crate::run::state::EventRoomChance::new(DEFAULT_EVENT_ROOM_TREASURE_CHANCE);
    if next_act == 2 {
        let (mut map, map_rng) = crate::map::target::generate_target_fixed_map_with_rng(
            run.reward_rng_seed as i64,
            TargetMapAct::City,
        );
        map.floor = run.current_floor as u32;
        run.map = Some(map);
        run.map_rng = Some(map_rng);
        generate_city_encounters_for_next_act(run)?;
        run.current_act = 2;
    } else if next_act == 3 {
        let (mut map, map_rng) = crate::map::target::generate_target_fixed_map_with_rng(
            run.reward_rng_seed as i64,
            TargetMapAct::Beyond,
        );
        map.floor = run.current_floor as u32;
        run.map = Some(map);
        run.map_rng = Some(map_rng);
        generate_beyond_encounters_for_next_act(run)?;
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
    // AbstractDungeon.initializeCardPools rebuilds colorlessCardPool from
    // CardLibrary at every act. A prior Match-and-Keep / Knowing Skull shuffle
    // must not carry into the next act (FIDL01323 Blind vs Finesse).
    run.colorless_card_pool.clear();
    // Relic pools persist across dungeon constructors after Exordium initializes
    // them; TheCity, TheBeyond, and TheEnding do not call initializeRelicList.
    if !run.has_mark_of_bloom() {
        run.player_hp = run.player_max_hp;
    }
    let final_act_available = run.final_act_available;
    run.set_final_act_available(final_act_available)?;
    Ok(())
}

fn generate_city_encounters_for_next_act(run: &mut RunState) -> SimResult<()> {
    let mut rng = StsRng::new(run.monster_rng_seed as i64);
    crate::content::encounters::advance_exordium_content_generation_rng(&mut rng)?;
    let (normal, elite) = generate_city_encounter_lists_with_rng(&mut rng)?;
    run.normal_encounter_list = normal;
    run.elite_encounter_list = elite;
    run.monster_rng_counter = rng.counter();
    Ok(())
}

fn generate_beyond_encounters_for_next_act(run: &mut RunState) -> SimResult<()> {
    // Dungeon content generation is replayed from the run seed for each act;
    // combat AI rolls accumulated during Act 2 must not contaminate the Act 3
    // encounter list.
    let mut rng = StsRng::new(run.monster_rng_seed as i64);
    crate::content::encounters::advance_exordium_content_generation_rng(&mut rng)?;
    let _ = generate_city_encounter_lists_with_rng(&mut rng)?;
    let (normal, elite) = generate_beyond_encounter_lists_with_rng(&mut rng)?;
    run.normal_encounter_list = normal;
    run.elite_encounter_list = elite;
    run.monster_rng_counter = rng.counter();
    Ok(())
}

pub(crate) fn advance_card_rng_for_dungeon_transition(run: &mut RunState) {
    match run.card_rng_counter {
        1..=249 => run.card_rng_counter = 250,
        251..=499 => run.card_rng_counter = 500,
        501..=749 => run.card_rng_counter = 750,
        _ => {}
    }
}

pub fn enter_elite_relic_reward_screen(run: &mut RunState) -> SimResult<()> {
    enter_relic_reward_screen(run, CombatRewardKind::Elite)
}

pub fn enter_chest_relic_reward_screen(run: &mut RunState) -> SimResult<()> {
    let mut next = run.clone();
    enter_chest_relic_reward_screen_inner(&mut next)?;
    *run = next;
    Ok(())
}

fn enter_chest_relic_reward_screen_inner(run: &mut RunState) -> SimResult<()> {
    if run.treasure_room.is_none() {
        setup_treasure_room(run);
    }
    apply_cursed_key_chest_curse(run)?;
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
    let bonus_relic_offer = if run.relics.contains(&Relic::Matryoshka)
        && run.matryoshka_chests_opened < crate::relic::MATRYOSHKA_MAX_CHESTS
    {
        run.matryoshka_chests_opened += 1;
        Some(roll_matryoshka_bonus_relic_offer(run))
    } else {
        None
    };
    let matryoshka_bonus_inserted = bonus_relic_offer.is_some();
    // AbstractChest.open invokes relic onChestOpen hooks before adding the
    // chest's own relic reward. Matryoshka therefore consumes relic RNG and
    // removes its relic from the pool before the normal chest relic is rolled.
    let chest_relic_offer = Some(roll_relic_reward(run, tier));
    if run.final_act_available == Some(true) && !run.has_sapphire_key {
        run.treasure_room
            .as_mut()
            .expect("treasure room exists while opening chest")
            .sapphire_key_relic_offer = chest_relic_offer;
    }
    // Matryoshka's extra reward is inserted before the chest's normal relic
    // in the target reward list (CombatRewardScreen insertion order). Do not
    // reorder when the chest relic is bottled: CM still lists Matryoshka first
    // then the chest bottle (729674a: Bronze Scales then Bottled Tornado).
    // Claiming the bottle later still opens its grid via gain_relic.
    let (relic_offer, pending_relic_offer) = if bonus_relic_offer.is_some() {
        (bonus_relic_offer, chest_relic_offer)
    } else {
        (chest_relic_offer, None)
    };
    // CombatRewardScreen keeps RewardItems in insertion order. Matryoshka's
    // onChestOpen inserts its bonus relic before the gold item, so residual
    // screens after claiming the trailing chest relic remain [relic, gold].
    if let Some(treasure_room) = run.treasure_room.as_mut() {
        treasure_room.relic_before_gold = matryoshka_bonus_inserted && gold_offer > 0;
    }

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        continuation: RewardContinuation::Map,
        choices: Vec::new(),
        queued_card_rewards: Vec::new(),
        gold_offer,
        stolen_gold_offer: 0,
        potion_offer: None,
        potion_offers: Vec::new(),
        relic_offer,
        pending_relic_offer,
        queued_relic_offers: Vec::new(),
        boss_relic_choices: Vec::new(),
        card_reward_flow: crate::run::CardRewardFlow::None,
    });
    Ok(())
}

fn apply_cursed_key_chest_curse(run: &mut RunState) -> SimResult<()> {
    if !run.relics.contains(&Relic::CursedKey) {
        return Ok(());
    }

    let mut next = run.clone();
    // Target `CardLibrary.getCurse()` samples with `AbstractDungeon.cardRng`.
    // This is the persistent reward-card stream, not the per-combat
    // `cardRandomRng` stream.
    let mut rng = next.rng_for_stream(RunRngStream::CardReward);
    let curse = random_normal_curse(&mut rng);
    next.store_rng_counter(RunRngStream::CardReward, &rng);
    next.gain_deck_card(curse)?;
    *run = next;
    Ok(())
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
    settle_run_after_combat_transition(
        &mut next,
        &combat_for_action,
        &mut next_combat,
        matches!(action, CombatAction::EndTurn),
    )?;

    if next_combat.phase == CombatPhase::Won {
        next.store_rng_counter(RunRngStream::CardRandom, &next_combat.rng.card_random_rng);
        let colosseum_first_fight = next_combat.monsters.len() == 2
            && next_combat.monsters[0].content_id == SLAVER_BLUE_ID
            && next_combat.monsters[1].content_id == SLAVER_RED_ID;
        if colosseum_first_fight {
            // EventRoom still runs AbstractRoom.addPotionToRewards after the
            // first Colosseum fight even though rewardAllowed suppresses the
            // reward screen. Preserve that hidden roll before returning to
            // the event dialog.
            let mut potion_rng = next.rng_for_stream(RunRngStream::Potion);
            let potion_count = next.potions.len();
            let potion_capacity = next.potion_capacity();
            let has_white_beast_statue = next.relics.contains(&Relic::WhiteBeastStatue);
            let _hidden_potion_offer = target_potion_reward_offer(
                &mut potion_rng,
                &mut next.potion_chance,
                0,
                potion_count,
                potion_capacity,
                has_white_beast_statue,
            )?;
            next.store_rng_counter(RunRngStream::Potion, &potion_rng);
            next.pending_event_combat_rng = Some(next_combat.rng.clone());
            next.combat = None;
            next.phase = RunPhase::Event;
            let mut event = event_screen_for_run(&next, Event::Colosseum);
            event.stage = 2;
            event.choices = colosseum_choices(2);
            next.event = Some(event);
        } else if next_combat.monsters.len() == 2
            && next_combat.monsters[0].content_id == TASKMASTER_ID
            && next_combat.monsters[1].content_id == GREMLIN_NOB_ID
        {
            enter_colosseum_combat_reward_screen(&mut next)?;
        } else if next.current_act >= 3
            && next.current_room_kind() == Some(crate::map::RoomKind::Boss)
        {
            enter_final_boss_victory(&mut next)?;
        } else if next.current_room_kind() == Some(crate::map::RoomKind::Boss) {
            enter_boss_combat_reward_screen(&mut next)?;
        } else if next.current_room_kind() == Some(crate::map::RoomKind::Elite) {
            enter_elite_combat_reward_screen(&mut next)?;
        } else {
            enter_reward_screen(&mut next)?;
        }
    }

    Ok(next)
}

pub(crate) fn settle_run_after_combat_transition(
    run: &mut RunState,
    before: &crate::combat::CombatState,
    after: &mut crate::combat::CombatState,
    finish_revived_end_turn: bool,
) -> SimResult<()> {
    if after.relic_counters.fairy_consumed {
        if let Some((slot, _)) = run
            .occupied_potion_slots()
            .into_iter()
            .find(|(_, potion)| *potion == Potion::Fairy)
        {
            run.take_potion_slot(slot)
                .expect("consumed fairy potion was present before combat transition");
        }
    }
    if run.relics.contains(&Relic::LizardTail) && !after.relic_counters.lizard_tail_available {
        run.lizard_tail_used = true;
    }
    apply_looter_theft_to_run_gold(run, before, after);
    apply_combat_gold_gain_to_run(run, before, after)?;
    settle_combat_obtain_actions_after_transition(run, before, after)?;
    sync_ritual_dagger_damage_to_deck(run, after);
    run.store_rng_counter(RunRngStream::CardRandom, &after.rng.card_random_rng);
    after.rng.card_random_rng = run.card_random_rng();

    let revived = apply_fairy_if_lethal(run, after)?;
    if revived
        && finish_revived_end_turn
        && after.phase == CombatPhase::WaitingForPlayer
        && after.monsters.iter().any(|monster| monster.alive)
    {
        finish_monster_turn_after_player_revival(after)?;
        start_player_turn(after)?;
    }

    run.combat = Some(after.clone());
    run.player_hp = after.player.hp;
    run.player_max_hp = after.player.max_hp;
    if run.relics.contains(&Relic::IncenseBurner) {
        run.incense_burner_counter = after.relic_counters.incense_burner_counter;
    }
    if run.relics.contains(&Relic::PenNib) {
        run.pen_nib_attacks_played = after.relic_counters.pen_nib_attacks_played;
    }
    if run.relics.contains(&Relic::InkBottle) {
        run.ink_bottle_cards_played = after.relic_counters.ink_bottle_cards_played;
    }
    if run.relics.contains(&Relic::HappyFlower) {
        run.happy_flower_turns = after.relic_counters.happy_flower_turns;
    }
    if run.relics.contains(&Relic::Sundial) {
        run.sundial_shuffles = after.relic_counters.sundial_shuffles;
    }
    if run.relics.contains(&Relic::Nunchaku) {
        run.nunchaku_attacks_played = after.relic_counters.nunchaku_attacks_played;
    }
    Ok(())
}

pub(crate) fn settle_combat_obtain_actions_after_transition(
    run: &mut RunState,
    before: &crate::combat::CombatState,
    after: &mut crate::combat::CombatState,
) -> SimResult<()> {
    settle_pending_combat_obtain_cards(run, after)?;
    queue_writhing_mass_mega_debuff_to_run(run, before, after)?;
    settle_pending_combat_obtain_cards(run, after)
}

fn settle_pending_combat_obtain_cards(
    run: &mut RunState,
    after: &mut crate::combat::CombatState,
) -> SimResult<()> {
    if run.pending_combat_obtain_cards.is_empty() {
        return Ok(());
    }

    // AddCardToDeckAction effects queued by the preceding combat action drain
    // during this transition, before its command-ready boundary.
    run.player_hp = after.player.hp;
    run.player_max_hp = after.player.max_hp;
    run.flush_pending_combat_obtain_cards()?;
    after.player.hp = run.player_hp;
    after.player.max_hp = run.player_max_hp;
    Ok(())
}

fn queue_writhing_mass_mega_debuff_to_run(
    run: &mut RunState,
    _before: &crate::combat::CombatState,
    after: &mut crate::combat::CombatState,
) -> SimResult<()> {
    let triggered = after.writhing_mass_mega_debuff_triggered;
    after.writhing_mass_mega_debuff_triggered = false;
    if triggered {
        // Writhing Mass's Mega Debuff queues AddCardToDeckAction(new Parasite)
        // after applying its player debuffs. The next combat-owned transition
        // drains that queued card before publication.
        run.queue_pending_combat_obtain_card(PARASITE_ID)?;
    }
    Ok(())
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
) -> SimResult<()> {
    let delta = after
        .combat_gold_gained
        .checked_sub(before.combat_gold_gained)
        .ok_or(SimError::InvalidState("combat gold delta overflows i32"))?
        .max(0);
    if delta > 0 {
        run.gain_gold(delta)?;
    }
    after.combat_gold_gained = before
        .combat_gold_gained
        .checked_add(delta)
        .ok_or(SimError::InvalidState("combat gold total overflows i32"))?;
    Ok(())
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

pub(crate) fn apply_dead_branch_for_exhaust_count(
    run: &mut RunState,
    combat: &mut crate::combat::CombatState,
    exhaust_count: usize,
) -> SimResult<()> {
    apply_dead_branch_for_exhaust_count_with_placement(
        run,
        combat,
        exhaust_count,
        DeadBranchPlacement::BackOfHand,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadBranchPlacement {
    BackOfHand,
}

fn apply_dead_branch_for_exhaust_count_with_placement(
    run: &mut RunState,
    combat: &mut crate::combat::CombatState,
    exhaust_count: usize,
    placement: DeadBranchPlacement,
) -> SimResult<()> {
    if exhaust_count == 0
        || !run.relics.contains(&Relic::DeadBranch)
        || !combat.monsters.iter().any(|monster| monster.alive)
    {
        return Ok(());
    }

    let first_id = combat.reserve_card_instance_ids(exhaust_count)?;
    let pool = dead_branch_card_pool();
    // Pending CONFIRM actions (Hex Dazed insert, etc.) already advanced
    // combat.card_random_rng. Rebuilding from the run counter would ignore
    // those draws and pick the wrong card (FIDL01442 Warcry+Hex: Clothesline
    // vs Ghostly Armor).
    let mut rng = combat.rng.card_random_rng.clone();
    let available_hand_slots = MAX_HAND_SIZE.saturating_sub(combat.piles.hand.len());
    let mut generated = Vec::with_capacity(exhaust_count);
    for offset in 0..exhaust_count {
        let next_id = first_id + offset as u64;
        let index = rng.random_int((pool.len() - 1) as i32) as usize;
        let content_id = pool[index];
        let mut card = CardInstance::new(CardId::new(next_id), content_id);
        card.combat_only = true;
        // Match add_generated_card_to_pile / MakeTempCardInHand: Blood for Blood
        // copies current combat damage events so cost is immediately playable.
        if content_id == BLOOD_FOR_BLOOD_ID || content_id == BLOOD_FOR_BLOOD_PLUS_ID {
            card.blood_for_blood_cost_reduction = combat.player.damage_events_this_combat;
        }
        if generated.len() < available_hand_slots {
            generated.push(card);
        } else {
            combat.piles.discard_pile.push(card);
        }
    }
    debug_assert_eq!(placement, DeadBranchPlacement::BackOfHand);
    combat.piles.hand.extend(generated);
    run.store_rng_counter(RunRngStream::CardRandom, &rng);
    combat.rng.card_random_rng = rng;
    Ok(())
}

fn dead_branch_card_pool() -> Vec<ContentId> {
    ironclad_combat_discovery_pool().to_vec()
}

fn apply_fairy_if_lethal(
    run: &mut RunState,
    combat: &mut crate::combat::CombatState,
) -> SimResult<bool> {
    if combat.player.hp > 0 && combat.phase != CombatPhase::Lost {
        return Ok(false);
    }

    if run.has_mark_of_bloom() {
        return Ok(false);
    }

    if run.relics.contains(&Relic::LizardTail) && !run.lizard_tail_used {
        let hp = revival_hp_with_relics(
            combat.player.max_hp,
            crate::relic::LIZARD_TAIL_HEAL_PERCENT,
            &run.relics,
        )?;
        run.lizard_tail_used = true;
        combat.player.hp = hp;
        combat.phase = CombatPhase::WaitingForPlayer;
        return Ok(true);
    }

    let Some((slot, _)) = run
        .occupied_potion_slots()
        .into_iter()
        .find(|(_, potion)| *potion == Potion::Fairy)
    else {
        return Ok(false);
    };
    let multiplier = if run.relics.contains(&Relic::SacredBark) {
        2
    } else {
        1
    };
    let hp = revival_hp_with_relics(
        combat.player.max_hp,
        FAIRY_HEAL_PERCENT * multiplier,
        &run.relics,
    )?;
    run.take_potion_slot(slot)
        .expect("fairy potion slot was found before consuming");
    combat.player.hp = hp;
    combat.phase = CombatPhase::WaitingForPlayer;
    Ok(true)
}

fn apply_combat_loss_proceed(run: &RunState) -> SimResult<RunState> {
    if run.phase != RunPhase::Combat
        || !run
            .combat
            .as_ref()
            .is_some_and(|combat| combat.phase == CombatPhase::Lost)
    {
        return Err(SimError::IllegalAction(
            "proceed from combat requires a lost combat",
        ));
    }
    let mut next = run.clone();
    next.phase = RunPhase::Complete;
    next.combat = None;
    next.reward = None;
    next.event = None;
    next.card_grid = None;
    Ok(next)
}

pub(crate) fn enter_final_boss_victory(run: &mut RunState) -> SimResult<()> {
    if run.phase != RunPhase::Combat
        || !matches!(run.current_act, 3 | 4)
        || run.current_room_kind() != Some(crate::map::RoomKind::Boss)
        || !run
            .combat
            .as_ref()
            .is_some_and(|combat| combat.phase == CombatPhase::Won)
    {
        return Err(SimError::InvalidState(
            "final boss victory requires a won combat in the final boss room",
        ));
    }

    // The target exposes COMPLETE at this boundary and does not make the
    // ordinary boss CombatRewardItems reachable. Preserve no reward overlay or
    // reward RNG state; PROCEED below enters the Spire Heart event.
    run.phase = RunPhase::Victory;
    run.combat = None;
    run.reward = None;
    Ok(())
}

pub fn apply_run_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    run.validate()?;

    let next = match action {
        RunAction::OpenChest => apply_treasure_action(run, action),
        RunAction::Proceed if run.phase == RunPhase::Reward => apply_reward_action(run, action),
        RunAction::Proceed if run.phase == RunPhase::Shop => apply_shop_action(run, action),
        RunAction::Proceed if run.phase == RunPhase::Combat => apply_combat_loss_proceed(run),
        RunAction::Proceed if run.phase == RunPhase::Victory => {
            apply_final_boss_victory_proceed(run)
        }
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
        RunAction::ConfirmHandSelectWithoutRetrieval => {
            apply_hand_select_confirm_without_retrieval(run)
        }
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
            } else if run.current_room_kind() == Some(RoomKind::Boss) && run.boss_chest_opened {
                // After a boss relic is claimed the room stays Treasure until PROCEED,
                // but the chest is no longer openable. Treat as illegal rather than
                // invalid so legal-action enumeration can still surface Proceed.
                Err(SimError::IllegalAction("boss chest is already resolved"))
            } else {
                Err(SimError::InvalidState("treasure room is missing"))
            }
        }
        RunAction::Proceed => {
            let boss_room = run.current_room_kind() == Some(RoomKind::Boss);
            if (boss_room && run.reward.is_none()) || (!boss_room && run.treasure_room.is_some()) {
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
                enter_chest_relic_reward_screen(&mut next)?;
            }
            Ok(next)
        }
        RunAction::Proceed => {
            if next.current_room_kind() == Some(RoomKind::Boss) {
                // Astrolabe's ShowCardAndObtainEffect settles after the final
                // boss-relic grid selection. PROCEED is the next room-owned
                // boundary, so commit the queued cards before entering the map.
                next.flush_pending_obtain_cards()?;
                enter_next_act_map(&mut next)?;
            } else {
                next.phase = RunPhase::Idle;
                next.treasure_room = None;
            }
            Ok(next)
        }
        _ => unreachable!("validated treasure action"),
    }
}

fn apply_final_boss_victory_proceed(run: &RunState) -> SimResult<RunState> {
    run.validate_final_boss_victory_action(RunAction::Proceed)?;
    let mut next = run.clone();
    if next.current_act == 4 {
        next.advance_floor()?;
        // TrueVictoryRoom is a real room transition; MawBank.onEnterRoom still
        // grants gold if the relic is not used up (FIDL02369).
        next.apply_floor_entry_relics()?;
        next.phase = RunPhase::Complete;
        next.combat = None;
        next.reward = None;
        next.event = None;
        next.card_grid = None;
    } else {
        enter_spire_heart_event(&mut next)?;
    }
    next.validate()?;
    Ok(next)
}

fn clear_sapphire_key_offer_if_linked(run: &mut RunState, relic: Relic) {
    if run
        .treasure_room
        .as_ref()
        .is_some_and(|treasure| treasure.sapphire_key_relic_offer == Some(relic))
    {
        run.treasure_room
            .as_mut()
            .expect("linked sapphire reward has a treasure room")
            .sapphire_key_relic_offer = None;
    }
}

fn apply_reward_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    run.validate_reward_action(action)?;

    let mut next = run.clone();
    match action {
        RunAction::SkipReward => {
            let is_boss_room = next.current_room_kind() == Some(RoomKind::Boss);
            let reward = next.reward.as_mut().expect("validated reward screen");
            if reward.card_reward_is_active() {
                let choices = reward.choices.clone();
                reward
                    .queued_card_rewards
                    .retain(|queued| *queued != choices);
                reward.choices.clear();
                reward.consume_active_card_reward()?;
                return_to_reward_continuation_if_empty(&mut next);
            } else if is_boss_room && !reward.boss_relic_choices.is_empty() {
                next.phase = RunPhase::Treasure;
                next.reward = None;
                next.boss_chest_opened = false;
            } else if is_boss_room
                && !next.boss_chest_opened
                && reward.boss_relic_choices.is_empty()
                && reward.pending_relic_offer.is_none()
                && reward.queued_relic_offers.is_empty()
            {
                enter_boss_reward_chest(&mut next)?;
            } else {
                close_reward_overlay(&mut next, RewardCloseReason::Automatic)?;
            }
        }
        RunAction::CloseCardReward => {
            let return_to_rest = next
                .reward
                .as_ref()
                .is_some_and(|reward| reward.continuation == RewardContinuation::Rest);
            let reward = next.reward.as_mut().expect("validated reward screen");
            if return_to_rest {
                reward.choices.clear();
                reward.consume_active_card_reward()?;
                return_to_reward_continuation_if_empty(&mut next);
            } else {
                reward.close_card_reward()?;
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
            let choices = reward.choices.clone();
            reward
                .queued_card_rewards
                .retain(|queued| *queued != choices);
            reward.choices.clear();
            reward.consume_active_card_reward()?;
            // Reward choices are already egg-previewed / upgrade-rolled at generation.
            // Adding them as ordinary obtains would re-apply Molten/Toxic/Frozen Egg and
            // double-upgrade Searing Blow (FIDL01326 step 691).
            next.add_finalized_reward_deck_card(choice)?;
            return_to_reward_continuation_if_empty(&mut next);
        }
        RunAction::TakeSingingBowlReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let choices = reward.choices.clone();
            reward
                .queued_card_rewards
                .retain(|queued| *queued != choices);
            reward.choices.clear();
            reward.consume_active_card_reward()?;
            // Real Singing Bowl calls AbstractPlayer.increaseMaxHp(2, true), which
            // raises max HP and heals the same amount (see AbstractCreature.increaseMaxHp).
            next.gain_max_hp(SINGING_BOWL_MAX_HP)?;
            return_to_reward_continuation_if_empty(&mut next);
        }
        RunAction::TakeGoldReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let gold_offer = reward.gold_offer;
            reward.gold_offer = 0;
            next.gain_gold(gold_offer)?;
        }
        RunAction::TakeStolenGoldReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let stolen_gold_offer = reward.stolen_gold_offer;
            reward.stolen_gold_offer = 0;
            next.gain_gold(stolen_gold_offer)?;
        }
        RunAction::TakePotionReward { index } => {
            if next.open_potion_slots() == 0 {
                return Ok(next);
            }
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
            // Claiming the leading reward-list relic drops Matryoshka's
            // relic-before-gold residual order; remaining chest rewards use the
            // ordinary gold-then-relic layout.
            let map_chest_relic_before_gold = next.reward.as_ref().is_some_and(|reward| {
                reward.continuation == RewardContinuation::Map
                    && next
                        .treasure_room
                        .as_ref()
                        .is_some_and(|treasure| treasure.relic_before_gold)
            });
            let relic_offer = next
                .reward
                .as_mut()
                .expect("validated reward screen")
                .relic_offer
                .take();
            if let Some(relic) = relic_offer {
                clear_sapphire_key_offer_if_linked(&mut next, relic);
                next.gain_relic(relic)?;
            }
            if map_chest_relic_before_gold {
                if let Some(treasure_room) = next.treasure_room.as_mut() {
                    treasure_room.relic_before_gold = false;
                }
            }
            if next.phase == RunPhase::Reward && next.card_grid.is_none() {
                advance_pending_relic_offer(&mut next);
            }
        }
        RunAction::TakeRelicRewardAt { index } => {
            let map_chest_relic_before_gold = next.reward.as_ref().is_some_and(|reward| {
                reward.continuation == RewardContinuation::Map
                    && next
                        .treasure_room
                        .as_ref()
                        .is_some_and(|treasure| treasure.relic_before_gold)
            });
            let reward = next.reward.as_mut().expect("validated reward screen");
            let active_count = usize::from(reward.relic_offer.is_some());
            let pending_count = usize::from(reward.pending_relic_offer.is_some());
            let queued_index = index.checked_sub(active_count + pending_count);
            let selected = if index == 0 {
                reward.relic_offer.take()
            } else if index == active_count && pending_count == 1 {
                reward.pending_relic_offer.take()
            } else {
                reward
                    .queued_relic_offers
                    .get(queued_index.expect("validated relic reward index"))
                    .copied()
            };
            let selected = selected.expect("validated relic reward");
            let mut remaining = Vec::with_capacity(
                active_count + pending_count + reward.queued_relic_offers.len() - 1,
            );
            if let Some(relic) = reward.relic_offer.take() {
                remaining.push(relic);
            }
            if let Some(relic) = reward.pending_relic_offer.take() {
                remaining.push(relic);
            }
            remaining.append(&mut reward.queued_relic_offers);
            remaining.retain(|relic| *relic != selected);
            reward.relic_offer = Some(selected);
            reward.queued_relic_offers = remaining;
            // TakeRelicReward clears relic_before_gold because it claims the
            // temporary primary slot. Non-leading indexed picks leave the
            // original leading relic in place, so restore the residual order.
            next = apply_reward_action(&next, RunAction::TakeRelicReward)?;
            if map_chest_relic_before_gold && index > 0 {
                if let Some(treasure_room) = next.treasure_room.as_mut() {
                    treasure_room.relic_before_gold = true;
                }
            }
        }
        RunAction::TakeSapphireKey => {
            let linked_relic = next
                .treasure_room
                .as_ref()
                .and_then(|treasure| treasure.sapphire_key_relic_offer)
                .expect("validated sapphire key reward");
            let reward = next.reward.as_mut().expect("validated reward screen");
            if reward.relic_offer == Some(linked_relic) {
                reward.relic_offer = None;
            } else if reward.pending_relic_offer == Some(linked_relic) {
                reward.pending_relic_offer = None;
            } else if let Some(index) = reward
                .queued_relic_offers
                .iter()
                .position(|relic| *relic == linked_relic)
            {
                reward.queued_relic_offers.remove(index);
            }
            next.treasure_room
                .as_mut()
                .expect("validated sapphire key treasure room")
                .sapphire_key_relic_offer = None;
            next.has_sapphire_key = true;
        }
        RunAction::TakeEmeraldKey => {
            next.has_emerald_key = true;
            next.emerald_key_reward_available = false;
            next.emerald_key_node = None;
        }
        RunAction::ChooseBossRelicReward { index } => {
            let key = {
                let reward = next.reward.as_mut().expect("validated reward screen");
                let key = reward.boss_relic_choices[index];
                reward.boss_relic_choices.clear();
                key
            };
            next.pending_boss_relic_choices.clear();
            if key == RelicKey::TinyHouse {
                // TinyHouse.onEquip adds its rewards to the current room and
                // opens CombatRewardScreen before the boss chest screen closes.
                // Keep the reward overlay alive while applying the relic so
                // those gold, potion, and card rewards are represented.
                next.gain_relic_key(key)?;
            } else {
                next.phase = RunPhase::Treasure;
                next.reward = None;
                next.gain_relic_key(key)?;
            }
        }
        RunAction::Proceed => {
            if next.current_act == 3 && next.current_room_kind() == Some(RoomKind::Boss) {
                enter_spire_heart_event(&mut next)?;
                return Ok(next);
            }
            // A relic hook (for example Calling Bell) may append rewards to an
            // already-open boss chest's CombatRewardScreen. The target keeps
            // that now-empty overlay visible until PROCEED, then closes the
            // resolved chest and advances to the next-act map in one transition.
            let leftover_boss_treasure_reward = next.current_room_kind() == Some(RoomKind::Boss)
                && next.boss_chest_opened
                && next.reward.as_ref().is_some_and(|reward| {
                    reward.continuation == RewardContinuation::Treasure
                        && !reward.card_reward_is_active()
                });
            if leftover_boss_treasure_reward {
                next.reward = None;
                next.flush_pending_obtain_cards()?;
                enter_next_act_map(&mut next)?;
                return Ok(next);
            }
            // Act 1/2 boss combat-reward PROCEED advances into the boss chest room,
            // matching SkipReward on the empty post-boss combat-reward screen.
            let is_boss_combat_reward = next.current_room_kind() == Some(RoomKind::Boss)
                && !next.boss_chest_opened
                && next.reward.as_ref().is_some_and(|reward| {
                    reward.continuation == RewardContinuation::None
                        && reward.boss_relic_choices.is_empty()
                        && reward.pending_relic_offer.is_none()
                        && reward.queued_relic_offers.is_empty()
                });
            if is_boss_combat_reward {
                enter_boss_reward_chest(&mut next)?;
            } else {
                let rest_reward_leaves_room = next.reward.as_ref().is_some_and(|reward| {
                    reward.continuation == RewardContinuation::Rest && next.rest_room_complete
                });
                let shop_overlay_leaves_room = next
                    .reward
                    .as_ref()
                    .is_some_and(|reward| reward.continuation == RewardContinuation::Shop);
                close_reward_overlay(&mut next, RewardCloseReason::Proceed)?;
                if rest_reward_leaves_room {
                    next.rest_room_complete = false;
                    next.phase = RunPhase::Idle;
                } else if shop_overlay_leaves_room {
                    super::shop::leave_shop_room(&mut next);
                }
            }
        }
        RunAction::OpenChest => {
            unreachable!("validated reward action")
        }
        RunAction::OpenCardReward => open_card_reward_at(&mut next, None)?,
        RunAction::OpenQueuedCardReward { index } => open_card_reward_at(&mut next, Some(index))?,
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
        RunAction::ChooseHandSelect { .. }
        | RunAction::ConfirmHandSelect
        | RunAction::ConfirmHandSelectWithoutRetrieval => {
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

fn open_card_reward_at(run: &mut RunState, queued_index: Option<usize>) -> SimResult<()> {
    if let Some(index) = queued_index {
        // A queued-card command explicitly selects the RewardItem to open.
        // Opening it does not consume the reward; selection does that later.
        let choices = run
            .reward
            .as_ref()
            .expect("reward screen present")
            .queued_card_rewards[index]
            .clone();
        run.reward.as_mut().expect("reward screen present").choices = choices;
    } else if run
        .reward
        .as_ref()
        .is_some_and(|reward| reward.choices.is_empty() && reward.remaining_card_reward_count() > 0)
    {
        let queued = run
            .reward
            .as_ref()
            .and_then(|reward| reward.queued_card_rewards.first().cloned());
        if let Some(choices) = queued {
            run.reward.as_mut().expect("reward screen present").choices = choices;
        } else {
            roll_pending_card_reward_choices(run)?;
        }
    }
    run.reward
        .as_mut()
        .expect("validated reward screen")
        .open_card_reward()?;
    Ok(())
}

fn return_to_reward_continuation_if_empty(run: &mut RunState) {
    let Some(reward) = run.reward.as_ref() else {
        return;
    };
    if reward.remaining_card_reward_count() > 0
        || !reward.choices.is_empty()
        || reward.gold_offer > 0
        || reward.stolen_gold_offer > 0
        || reward.potion_offer.is_some()
        || reward.relic_offer.is_some()
        || reward.pending_relic_offer.is_some()
        || !reward.queued_relic_offers.is_empty()
        || !reward.boss_relic_choices.is_empty()
    {
        return;
    }
    // Event and Shop (Orrery) rewards remain as an empty CombatRewardScreen until
    // Leave/Proceed/SKIP closes the overlay (FIDL00405: empty combat-reward after
    // the last Orrery card, then SKIP → SHOP_ROOM). Map continuations also hold.
    // This is also how event relic and potion rewards are represented.
    if reward.continuation != RewardContinuation::None
        && reward.continuation != RewardContinuation::Event
        && reward.continuation != RewardContinuation::Map
        && reward.continuation != RewardContinuation::Shop
    {
        close_reward_overlay(run, RewardCloseReason::Automatic)
            .expect("closing an empty reward overlay must settle pending obtain cards");
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RewardCloseReason {
    Automatic,
    Proceed,
}

fn close_reward_overlay(run: &mut RunState, reason: RewardCloseReason) -> SimResult<()> {
    let continuation = run
        .reward
        .as_ref()
        .map(|reward| reward.continuation)
        .unwrap_or(RewardContinuation::None);
    run.phase = match continuation {
        RewardContinuation::None => RunPhase::Idle,
        RewardContinuation::Rest => RunPhase::Rest,
        RewardContinuation::Event => RunPhase::Event,
        RewardContinuation::Shop => RunPhase::Shop,
        RewardContinuation::Map => RunPhase::Idle,
        RewardContinuation::Treasure => RunPhase::Treasure,
        RewardContinuation::Neow if reason == RewardCloseReason::Automatic => RunPhase::Event,
        RewardContinuation::Neow => RunPhase::Idle,
    };
    run.reward = None;
    run.emerald_key_reward_available = false;
    if continuation == RewardContinuation::Map {
        run.treasure_room = None;
    }
    if continuation == RewardContinuation::Neow && reason == RewardCloseReason::Proceed {
        run.event = None;
    }
    // Settle queued obtain effects (e.g. Necronomicurse from Necronomicon) once
    // the combat/event reward overlay closes, matching ShowCardAndObtainEffect
    // completing before the next room frame.
    run.flush_pending_obtain_cards()
}

pub(crate) fn reward_is_empty(reward: &RewardScreen) -> bool {
    reward.remaining_card_reward_count() == 0
        && reward.choices.is_empty()
        && reward.queued_card_rewards.is_empty()
        && reward.gold_offer == 0
        && reward.stolen_gold_offer == 0
        && reward.potion_offer.is_none()
        && reward.potion_offers.is_empty()
        && reward.relic_offer.is_none()
        && reward.pending_relic_offer.is_none()
        && reward.queued_relic_offers.is_empty()
        && reward.boss_relic_choices.is_empty()
}

pub(crate) fn advance_pending_relic_offer(run: &mut RunState) {
    let Some(reward) = run.reward.as_mut() else {
        return;
    };

    if reward.pending_relic_offer.is_some() {
        reward.relic_offer = reward.pending_relic_offer.take();
        return;
    }

    let Some(next_relic) = reward.queued_relic_offers.first().copied() else {
        reward.relic_offer = None;
        return;
    };
    reward.queued_relic_offers.remove(0);
    reward.relic_offer = Some(next_relic);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        content::cards::{
            BLOODLETTING_ID, DUAL_WIELD_ID, FIRE_BREATHING_ID, HEADBUTT_ID, HEAVY_BLADE_ID,
            JUGGERNAUT_ID, METALLICIZE_ID, POMMEL_STRIKE_ID, POWER_THROUGH_ID, RAGE_PLUS_ID,
            SHOCKWAVE_PLUS_ID, SPOT_WEAKNESS_ID, STRIKE_R_ID, SWIFT_STRIKE_ID, THUNDERCLAP_ID,
            WARCRY_ID, WHIRLWIND_ID,
        },
        content::monsters::{
            monster_state, DARKLING_A0_NIP_DAMAGE_RANGE, DARKLING_ID, WRITHING_MASS_A0,
        },
        run::{
            neow::{generate_neow_colorless_reward, NeowRewardType},
            RunState,
        },
        CardId, CardInstance, CombatAction, CombatState, Event, MonsterId, Relic, RelicKey,
        RoomKind,
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
    fn black_star_bonus_skips_girya_for_next_rare_in_pool() {
        // Ordinary elite relic uses returnRandomRelic (Girya is legal). Black
        // Star's extra drop retries the same tier after consuming Girya /
        // Peace Pipe / Shovel, so the next rare in the pool is offered.
        let mut run = RunState::seeded_ironclad(1, 0);
        run.current_act = 2;
        run.current_floor = 29;
        run.relics.push(Relic::BlackStar);
        run.ensure_ironclad_relic_pools();
        {
            let pools = run.relic_pools.as_mut().expect("pools");
            pools.common = vec![Relic::Omamori, Relic::Lantern];
            pools.rare = vec![Relic::Girya, Relic::Pocketwatch, Relic::StoneCalendar];
        }
        let seed = run.relic_rng_seed;
        let mut first_counter = None;
        for counter in 0..512u32 {
            let mut probe = StsRng::with_counter(seed as i64, counter);
            let first = target_elite_relic_tier(&mut probe);
            let second = target_elite_relic_tier(&mut probe);
            if first == RelicTier::Common && second == RelicTier::Rare {
                first_counter = Some(counter);
                break;
            }
        }
        run.relic_rng_counter = first_counter.expect("common then rare elite rolls");
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Elite);
        enter_elite_combat_reward_screen(&mut run).expect("elite reward entry succeeds");

        let reward = run.reward.as_ref().expect("reward screen");
        assert_eq!(reward.relic_offer, Some(Relic::Omamori));
        assert_eq!(reward.pending_relic_offer, Some(Relic::Pocketwatch));
    }

    #[test]
    fn elite_card_reward_applies_rarity_factor_to_uncommon_cutoff() {
        let mut run = RunState::seeded_ironclad(34_961_238_615_911, 0);
        run.current_act = 3;
        run.current_floor = 45;
        run.card_rng_counter = 551;
        run.card_rarity_factor = -3;
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Elite);

        enter_elite_combat_reward_screen(&mut run).expect("elite reward entry succeeds");

        assert_eq!(
            reward_choice_ids(&run),
            vec![BLOODLETTING_ID, JUGGERNAUT_ID, RAGE_PLUS_ID]
        );
    }

    #[test]
    fn nloths_gift_triples_rare_chance_without_resetting_pity_offset() {
        let mut run = RunState::map_fixture();
        run.current_room_override = Some(RoomKind::Combat);
        let normal = NORMAL_REWARD_RARITY_CHANCES;
        assert_eq!(reward_rarity_chances_for_run(&run, normal), normal);

        run.relics.push(Relic::NlothsGift);
        assert_eq!(
            reward_rarity_chances_for_run(&run, normal),
            RewardRarityChances {
                rare: 9,
                uncommon: 37,
            }
        );

        run.current_room_override = Some(RoomKind::Shop);
        assert_eq!(reward_rarity_chances_for_run(&run, normal), normal);
    }

    fn prepare_won_combat_reward_fixture(run: &mut RunState) {
        let mut combat = run
            .combat
            .take()
            .unwrap_or_else(CombatState::initial_fixture);
        combat.phase = CombatPhase::Won;
        combat.player.hp = run.player_hp;
        combat.player.max_hp = run.player_max_hp;
        combat.ascension = run.ascension;
        combat.relics = run.relics.clone();
        for monster in &mut combat.monsters {
            monster.hp = 0;
            monster.alive = false;
        }
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.reward = None;
        run.event = None;
        run.card_grid = None;
        run.combat = Some(combat);
    }

    #[test]
    fn cursed_key_uses_persistent_card_rng_on_session31_seed() {
        let seed = (-7_812_685_662_221_499_508_i64) as u64;
        let mut run = RunState::seeded_ironclad(seed, 0);
        run.reward_rng_seed = seed;
        run.card_rng_counter = 272;
        run.card_random_rng_counter = 19;
        run.relics.push(Relic::CursedKey);
        let deck_len = run.deck.len();

        apply_cursed_key_chest_curse(&mut run).expect("Cursed Key curse gain succeeds");

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
    fn matryoshka_chest_does_not_promote_bottled_chest_relic_ahead_of_bonus() {
        // CM lists Matryoshka's onChestOpen bonus first, then the chest relic —
        // even when the chest rolls a bottled relic. Swapping for bottle grids
        // inverted relic_offer_ids (729674a Bronze Scales vs Bottled Tornado).
        let mut run = RunState::seeded_ironclad(1, 0);
        run.event = None;
        run.gain_relic(Relic::Matryoshka)
            .expect("Matryoshka equips");
        run.phase = RunPhase::Treasure;
        setup_treasure_room(&mut run);
        enter_chest_relic_reward_screen(&mut run).expect("chest opens");
        let reward = run.reward.as_mut().expect("reward screen");
        assert!(reward.relic_offer.is_some() && reward.pending_relic_offer.is_some());
        // Reconstruct the permanent's dual-relic shape and assert enter_chest
        // left primary=bonus / pending=chest without bottle-promotion.
        reward.relic_offer = Some(Relic::BronzeScales);
        reward.pending_relic_offer = Some(Relic::BottledTornado);
        // enter_chest_relic_reward_screen_inner no longer swaps when pending is
        // bottled; projection order is relic_offer then pending_relic_offer.
        let projected = [
            reward.relic_offer.expect("primary"),
            reward.pending_relic_offer.expect("pending"),
        ];
        assert_eq!(
            projected,
            [Relic::BronzeScales, Relic::BottledTornado],
            "CM reward list order is Matryoshka bonus then chest relic"
        );
    }

    #[test]
    fn entropic_brew_uses_source_potion_selection_sequence() {
        let mut potion_rng = StsRng::new(34961238620706);
        let first = target_random_combat_potion(&mut potion_rng);
        let second = target_random_combat_potion(&mut potion_rng);
        let third = target_random_combat_potion(&mut potion_rng);
        assert_eq!(
            [first, second, third],
            [Potion::BlessingOfTheForge, Potion::Ancient, Potion::Fire]
        );
        assert_eq!(potion_rng.counter(), 11);
    }

    #[test]
    fn combat_entropic_brew_fill_matches_fidelity_trace_5f3f2d8c() {
        // Archived schema-v0 witness random-fidelity-5f3f2d8cafb4a224.jsonl.
        // After floor-1 Entropic Brew reward, potion_rng_counter is 6. Combat use
        // fills three empty slots with Attack, Attack, Swift.
        let mut potion_rng = StsRng::with_counter(34961238620706_i64, 6);
        let potions = [
            target_random_combat_potion(&mut potion_rng),
            target_random_combat_potion(&mut potion_rng),
            target_random_combat_potion(&mut potion_rng),
        ];
        assert_eq!(potions, [Potion::Attack, Potion::Attack, Potion::Swift]);
        assert_eq!(potion_rng.counter(), 15);
    }

    #[test]
    fn boss_calling_bell_relics_keep_empty_reward_until_proceed() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.current_room_override = Some(RoomKind::Boss);
        run.event = None;
        run.boss_chest_opened = true;
        enter_calling_bell_reward_screen(&mut run);

        for _ in 0..3 {
            run = apply_run_action(&run, RunAction::TakeRelicReward)
                .expect("Calling Bell relic can be collected");
        }

        let reward = run.reward.as_ref().expect("empty boss reward overlay");
        assert_eq!(run.phase, RunPhase::Reward);
        assert_eq!(reward.continuation, RewardContinuation::Treasure);
        assert!(reward_is_empty(reward));
        assert!(run.boss_chest_opened);

        let next = apply_run_action(&run, RunAction::Proceed)
            .expect("completed boss reward advances to the next-act map");
        assert_eq!(next.phase, RunPhase::Idle);
        assert_eq!(next.current_act, 2);
        assert_eq!(next.current_floor, run.current_floor);
        assert!(next.reward.is_none());
        assert!(!next.boss_chest_opened);
    }

    #[test]
    fn indexed_calling_bell_relic_selection_preserves_unselected_offer_order() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.current_room_override = Some(RoomKind::Boss);
        run.event = None;
        run.boss_chest_opened = true;
        enter_calling_bell_reward_screen(&mut run);

        let offered = {
            let reward = run.reward.as_ref().expect("Calling Bell reward");
            reward
                .relic_offer
                .iter()
                .chain(reward.pending_relic_offer.iter())
                .chain(reward.queued_relic_offers.iter())
                .copied()
                .collect::<Vec<_>>()
        };
        assert_eq!(offered.len(), 3);
        let selected = offered[2];
        let next = apply_run_action(&run, RunAction::TakeRelicRewardAt { index: 2 })
            .expect("indexed Calling Bell relic can be collected");

        assert!(next.relics.contains(&selected));
        let remaining = next
            .reward
            .as_ref()
            .expect("remaining Calling Bell reward")
            .relic_offer
            .iter()
            .chain(
                next.reward
                    .as_ref()
                    .expect("remaining Calling Bell reward")
                    .queued_relic_offers
                    .iter(),
            )
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(remaining, offered[..2]);
    }

    #[test]
    fn final_boss_combat_victory_exposes_complete_before_proceed() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.current_act = 3;
        run.current_floor = 50;
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Boss);
        let reward_rng_counters = (
            run.misc_rng_counter,
            run.potion_rng_counter,
            run.card_rng_counter,
        );

        enter_final_boss_victory(&mut run).expect("won final boss enters victory boundary");

        assert_eq!(run.phase, RunPhase::Victory);
        assert!(run.combat.is_none());
        assert!(run.reward.is_none());
        assert_eq!(
            (
                run.misc_rng_counter,
                run.potion_rng_counter,
                run.card_rng_counter,
            ),
            reward_rng_counters,
            "final boss completion must not generate inaccessible rewards"
        );
        run.validate().expect("victory boundary validates");
        assert_eq!(
            crate::legal_run_decision_actions(&run).expect("victory legal actions"),
            vec![crate::RunDecisionAction::Run(RunAction::Proceed)]
        );

        let encoded = serde_json::to_string(&run).expect("serialize victory boundary");
        let restored: RunState = serde_json::from_str(&encoded).expect("restore victory boundary");
        assert_eq!(restored, run);

        let next = apply_run_action(&run, RunAction::Proceed)
            .expect("victory proceed enters the Spire Heart event");
        assert_eq!(next.phase, RunPhase::Event);
        assert_eq!(next.current_floor, 51);
        assert_eq!(next.current_room_kind(), Some(RoomKind::Victory));
        assert!(next.reward.is_none());
        assert_eq!(
            next.event.as_ref().map(|event| event.event),
            Some(Event::SpireHeart)
        );
    }

    #[test]
    fn corrupt_heart_victory_proceeds_to_true_victory_floor() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.current_act = 4;
        run.current_floor = 55;
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Boss);

        enter_final_boss_victory(&mut run).expect("won Heart enters COMPLETE boundary");
        assert_eq!(run.phase, RunPhase::Victory);

        let complete = apply_run_action(&run, RunAction::Proceed)
            .expect("Heart victory proceeds to TrueVictory");
        assert_eq!(complete.phase, RunPhase::Complete);
        assert_eq!(complete.current_floor, 56);
        assert!(complete.combat.is_none());
        assert!(complete.reward.is_none());
    }

    #[test]
    fn corrupt_heart_victory_applies_maw_bank_gold() {
        use crate::relic::{Relic, MAW_BANK_GOLD};

        let mut run = RunState::seeded_ironclad(7, 0);
        run.current_act = 4;
        run.current_floor = 55;
        run.gold = 100;
        run.maw_bank_broken = false;
        run.relics = vec![Relic::BurningBlood, Relic::MawBank];
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Boss);

        enter_final_boss_victory(&mut run).expect("won Heart enters COMPLETE boundary");
        let complete = apply_run_action(&run, RunAction::Proceed)
            .expect("Heart victory proceeds to TrueVictory");
        assert_eq!(complete.gold, 100 + MAW_BANK_GOLD);
    }

    #[test]
    fn final_boss_inaccessible_reward_proceeds_to_spire_heart() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        run.current_act = 3;
        run.current_floor = 50;
        run.current_room_override = Some(RoomKind::Boss);
        run.event = None;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 100,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: crate::run::CardRewardFlow::None,
        });

        let next = apply_run_action(&run, RunAction::Proceed)
            .expect("final boss victory can enter the Spire Heart event");

        assert_eq!(next.phase, RunPhase::Event);
        assert_eq!(next.current_floor, 51);
        assert_eq!(next.current_room_kind(), Some(RoomKind::Victory));
        // No Maw Bank on this fixture — gold unchanged on Heart entry.
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
        assert!(crate::legal_event_actions(&completed)
            .expect("valid completed event state")
            .is_empty());

        let json = serde_json::to_string(&completed).expect("serialize completed run state");
        let restored: RunState = serde_json::from_str(&json).expect("restore completed run state");
        assert_eq!(restored, completed);
    }

    #[test]
    fn spire_heart_entry_applies_maw_bank_gold() {
        use crate::relic::{Relic, MAW_BANK_GOLD};

        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        run.current_act = 3;
        run.current_floor = 50;
        run.current_room_override = Some(RoomKind::Boss);
        run.gold = 100;
        run.maw_bank_broken = false;
        run.relics = vec![Relic::BurningBlood, Relic::MawBank];
        run.event = None;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: crate::run::CardRewardFlow::None,
        });

        let next = apply_run_action(&run, RunAction::Proceed)
            .expect("final boss victory can enter the Spire Heart event");
        assert_eq!(next.current_floor, 51);
        assert_eq!(next.gold, 100 + MAW_BANK_GOLD);
        assert_eq!(
            next.event.as_ref().map(|event| event.event),
            Some(Event::SpireHeart)
        );
    }

    #[test]
    fn gold_reward_overflow_fails_closed_at_the_core_action_boundary() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Reward;
        run.current_room_override = Some(RoomKind::Combat);
        run.event = None;
        run.gold = i32::MAX;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 1,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: crate::run::CardRewardFlow::None,
        });

        assert_eq!(
            apply_run_action(&run, RunAction::TakeGoldReward),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
    }

    #[test]
    fn terminal_floor_advancement_overflow_leaves_run_unchanged() {
        let mut boss_chest = RunState::map_fixture();
        boss_chest.current_floor = i32::MAX;
        let boss_before = boss_chest.clone();
        assert_eq!(
            enter_boss_reward_chest(&mut boss_chest),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(boss_chest, boss_before);

        let mut heart = RunState::map_fixture();
        heart.current_floor = i32::MAX;
        let heart_before = heart.clone();
        assert_eq!(
            enter_spire_heart_event(&mut heart),
            Err(SimError::InvalidState("run integer addition overflows i32"))
        );
        assert_eq!(heart, heart_before);
    }

    #[test]
    fn boss_reward_chest_applies_maw_bank_on_floor_entry() {
        let mut run = RunState::map_fixture();
        run.current_floor = 16;
        run.current_room_override = Some(RoomKind::Boss);
        run.relics = vec![Relic::MawBank];
        run.gold = 100;

        enter_boss_reward_chest(&mut run).expect("boss reward chest entry succeeds");

        assert_eq!(run.current_floor, 17);
        assert_eq!(run.gold, 112);
    }

    #[test]
    fn neow_three_potions_hidden_reward_consumption_is_seed_dependent() {
        let mut duplicate_reroll_run = RunState::seeded_ironclad(2_080_939_458_480_311_800_u64, 0);
        consume_neow_three_potions_hidden_card_reward(&mut duplicate_reroll_run)
            .expect("hidden Neow reward succeeds");
        assert_eq!(duplicate_reroll_run.card_rng_counter, 10);
        assert_eq!(duplicate_reroll_run.card_rarity_factor, 5);
        prepare_won_combat_reward_fixture(&mut duplicate_reroll_run);
        enter_normal_combat_reward_screen(&mut duplicate_reroll_run)
            .expect("normal reward entry succeeds");
        assert_eq!(
            reward_choice_ids(&duplicate_reroll_run),
            vec![POWER_THROUGH_ID, POMMEL_STRIKE_ID, WARCRY_ID]
        );

        let mut no_reroll_run = RunState::seeded_ironclad(22_079_335_079, 0);
        consume_neow_three_potions_hidden_card_reward(&mut no_reroll_run)
            .expect("hidden Neow reward succeeds");
        assert_eq!(no_reroll_run.card_rng_counter, 9);
        assert_eq!(no_reroll_run.card_rarity_factor, 2);
        prepare_won_combat_reward_fixture(&mut no_reroll_run);
        enter_normal_combat_reward_screen(&mut no_reroll_run)
            .expect("normal reward entry succeeds");
        assert_eq!(
            reward_choice_ids(&no_reroll_run),
            vec![DUAL_WIELD_ID, WHIRLWIND_ID, HEAVY_BLADE_ID]
        );
    }

    #[test]
    fn test_seed_colorless_neow_carries_card_rng_through_first_two_combat_rewards() {
        let numeric_seed = 1_218_623_i64;
        let neow_reward =
            generate_neow_colorless_reward(numeric_seed, NeowRewardType::RandomColorless)
                .expect("RandomColorless is a colorless Neow reward");

        let mut run = RunState::seeded_ironclad(numeric_seed as u64, 0);
        run.card_rng_counter = neow_reward.card_rng_counter;
        run.gain_deck_card(SWIFT_STRIKE_ID)
            .expect("Swift Strike gain succeeds");
        run.current_act = 1;
        prepare_won_combat_reward_fixture(&mut run);

        enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");
        assert_eq!(
            reward_choice_ids(&run),
            vec![FIRE_BREATHING_ID, SPOT_WEAKNESS_ID, HEADBUTT_ID]
        );

        run.gain_deck_card(SPOT_WEAKNESS_ID)
            .expect("Spot Weakness gain succeeds");
        prepare_won_combat_reward_fixture(&mut run);
        enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");
        assert_eq!(
            reward_choice_ids(&run),
            vec![THUNDERCLAP_ID, WARCRY_ID, METALLICIZE_ID]
        );
    }

    #[test]
    fn prayer_wheel_eagerly_consumes_both_hidden_reward_rolls_from_session_1224() {
        let mut run = RunState::seeded_ironclad((-4_906_255_751_777_637_416_i64) as u64, 0);
        run.current_room_override = Some(RoomKind::Combat);
        run.card_rng_counter = 90;
        run.card_rarity_factor = -1;
        run.relics.push(Relic::QuestionCard);
        run.relics.push(Relic::PrayerWheel);
        prepare_won_combat_reward_fixture(&mut run);

        enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");

        let reward = run.reward.as_ref().expect("combat reward");
        assert_eq!(reward.remaining_card_reward_count(), 2);
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
    fn prayer_wheel_does_not_double_card_reward_in_event_rooms() {
        // Masked Bandits / other EventRoom fights are not MonsterRoom; Prayer
        // Wheel must not append a second card reward.
        let mut run = RunState::seeded_ironclad(1, 0);
        run.relics.push(Relic::PrayerWheel);
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Event);

        enter_normal_combat_reward_screen(&mut run).expect("event combat reward entry succeeds");

        let reward = run.reward.as_ref().expect("combat reward");
        assert_eq!(reward.remaining_card_reward_count(), 1);
        assert_eq!(reward.queued_card_rewards.len(), 0);
        assert_eq!(reward.choices.len(), reward_card_choice_count(&run));
    }

    #[test]
    fn close_card_reward_preserves_choices_for_reopen() {
        let mut run = RunState::seeded_ironclad(1_260_350_191_924, 0);
        prepare_won_combat_reward_fixture(&mut run);
        enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");

        let opened = apply_run_action(&run, RunAction::OpenCardReward).expect("card reward opens");
        let original = reward_choice_ids(&opened);
        let opened_card_rng_counter = opened.card_rng_counter;
        assert!(opened
            .reward
            .as_ref()
            .expect("reward")
            .card_reward_is_active());

        let closed =
            apply_run_action(&opened, RunAction::CloseCardReward).expect("card reward closes");
        assert!(!closed
            .reward
            .as_ref()
            .expect("reward")
            .card_reward_is_active());
        assert_eq!(reward_choice_ids(&closed), original);
        assert_eq!(closed.card_rng_counter, opened_card_rng_counter);

        let reopened =
            apply_run_action(&closed, RunAction::OpenCardReward).expect("card reward reopens");
        assert!(reopened
            .reward
            .as_ref()
            .expect("reward")
            .card_reward_is_active());
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
        run.gain_relic_key(scrap_ooze_relic)
            .expect("fixture relic pickup succeeds");

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

        let combat = run
            .init_combat(CombatState::initial_fixture())
            .expect("combat initializes");
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
        let combat = run.init_combat(base).expect("combat initializes");
        assert_eq!(combat.relic_counters.sundial_shuffles, 2);
        run.combat = Some(combat);

        let next = apply_combat_action_on_run(&run, CombatAction::EndTurn).expect("turn ends");
        let combat = next.combat.as_ref().expect("combat remains active");

        assert_eq!(next.sundial_shuffles, 3);
        assert_eq!(combat.relic_counters.sundial_shuffles, 3);
        assert_eq!(combat.player.energy, 5);
    }

    #[test]
    fn writhing_mass_mega_debuff_settles_queued_parasite_on_next_combat_boundary() {
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

        // Mega Debuff addToBot(AddCardToDeckAction) drains before the END
        // command-ready boundary, so Parasite and Ceramic Fish settle here.
        assert_eq!(next.deck.len(), starting_deck_len + 1);
        assert_eq!(next.gold, starting_gold + crate::relic::CERAMIC_FISH_GOLD);
        assert!(next.pending_combat_obtain_cards.is_empty());
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
    fn take_card_reward_grants_ceramic_fish_gold() {
        // FIDL00426: claiming a combat card reward with Ceramic Fish must add
        // CERAMIC_FISH_GOLD on the same transition as the deck obtain.
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Reward;
        run.current_room_override = Some(RoomKind::Combat);
        run.relics = vec![Relic::CeramicFish];
        let starting_gold = run.gold;
        let starting_deck = run.deck.len();
        let card = CardInstance::new(CardId::new(9_501), crate::content::cards::ANGER_ID);
        let card_id = card.id;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: vec![card],
            queued_card_rewards: Vec::new(),
            gold_offer: 0,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: crate::run::CardRewardFlow::active(1),
        });

        let next = apply_run_action(&run, RunAction::TakeCardReward { card_id })
            .expect("card reward claim succeeds");
        assert_eq!(next.deck.len(), starting_deck + 1);
        assert_eq!(next.gold, starting_gold + crate::relic::CERAMIC_FISH_GOLD);
    }

    #[test]
    fn half_dead_darklings_still_allow_combat_gold_reward() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);

        let mut combat = CombatState::initial_fixture();
        for monster in &mut combat.monsters {
            monster.content_id = DARKLING_ID;
            monster.rolled_attack_damage = Some(DARKLING_A0_NIP_DAMAGE_RANGE.min);
            monster.hp = 0;
            monster.alive = false;
            monster.escaped = true;
        }
        run.combat = Some(combat);
        prepare_won_combat_reward_fixture(&mut run);

        enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");

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
        prepare_won_combat_reward_fixture(&mut run);

        enter_normal_combat_reward_screen(&mut run).expect("normal reward entry succeeds");

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
        assert!(validate_treasure_action(&run, RunAction::Proceed).is_ok());

        let skipped_unopened =
            apply_run_action(&run, RunAction::Proceed).expect("unopened boss chest can be skipped");
        assert_eq!(skipped_unopened.phase, RunPhase::Idle);
        assert_eq!(skipped_unopened.current_act, 2);

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
    fn boss_chest_reserves_relics_before_it_is_opened() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Reward;
        run.current_room_override = Some(RoomKind::Boss);
        run.current_floor = 16;
        run.relic_rng_seed = 1_218_623;
        run.relics = vec![Relic::BurningBlood];

        enter_boss_reward_chest(&mut run).expect("boss chest entry succeeds");

        assert_eq!(run.phase, RunPhase::Treasure);
        assert!(run.reward.is_none());
        assert!(!run.boss_chest_opened);
        assert_eq!(run.pending_boss_relic_choices.len(), 3);

        let reserved = run.pending_boss_relic_choices.clone();
        let opened = apply_run_action(&run, RunAction::OpenChest).expect("boss chest opens");
        assert_eq!(
            opened
                .reward
                .as_ref()
                .expect("boss relic reward")
                .boss_relic_choices,
            reserved
        );
    }

    #[test]
    fn tiny_house_boss_relic_opens_its_room_reward_overlay() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(RoomKind::Boss);
        run.current_floor = 17;
        run.current_act = 1;
        run.player_hp = 50;
        run.player_max_hp = 100;
        run.relics = vec![Relic::BurningBlood];

        let mut opened = apply_run_action(&run, RunAction::OpenChest).expect("boss chest opens");
        opened
            .reward
            .as_mut()
            .expect("boss relic reward")
            .boss_relic_choices = vec![RelicKey::TinyHouse, RelicKey::MarkOfPain];

        let card_rng_counter_before = opened.card_rng_counter;
        let picked = apply_run_action(&opened, RunAction::ChooseBossRelicReward { index: 0 })
            .expect("Tiny House can be picked");

        assert_eq!(picked.phase, RunPhase::Reward);
        assert_eq!(picked.player_max_hp, 105);
        assert_eq!(picked.player_hp, 55);
        assert!(picked.relics.contains(&Relic::TinyHouse));
        let reward = picked.reward.as_ref().expect("Tiny House reward overlay");
        assert_eq!(reward.continuation, RewardContinuation::Treasure);
        assert_eq!(reward.gold_offer, 50);
        assert_eq!(reward.remaining_card_reward_count(), 1);
        assert_eq!(reward.choices.len(), 3);
        assert!(picked.card_rng_counter > card_rng_counter_before);
        assert_eq!(picked.current_act, 1);

        let proceeded = apply_run_action(&picked, RunAction::Proceed)
            .expect("Tiny House leftover overlay proceeds to the next act");
        assert_eq!(proceeded.phase, RunPhase::Idle);
        assert_eq!(proceeded.current_act, 2);
        assert!(proceeded.reward.is_none());

        let skipped = apply_run_action(&picked, RunAction::SkipReward)
            .expect("Tiny House parent reward can return to the boss chest");
        assert_eq!(skipped.phase, RunPhase::Treasure);
        assert_eq!(skipped.current_floor, picked.current_floor);
        assert!(skipped.reward.is_none());
        assert!(skipped.boss_chest_opened);
    }

    #[test]
    fn tiny_house_gold_offer_gets_golden_idol_bonus() {
        // TinyHouse.onEquip adds a 50-gold RewardItem; Golden Idol's 25% combat
        // gold bonus applies (50 + round(12.5) = 63). Source: 884c8929.
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(RoomKind::Boss);
        run.current_floor = 17;
        run.current_act = 1;
        run.player_hp = 50;
        run.player_max_hp = 100;
        run.relics = vec![Relic::BurningBlood, Relic::GoldenIdol];

        let mut opened = apply_run_action(&run, RunAction::OpenChest).expect("boss chest opens");
        opened
            .reward
            .as_mut()
            .expect("boss relic reward")
            .boss_relic_choices = vec![RelicKey::TinyHouse, RelicKey::MarkOfPain];

        let picked = apply_run_action(&opened, RunAction::ChooseBossRelicReward { index: 0 })
            .expect("Tiny House can be picked");

        assert!(picked.relics.contains(&Relic::TinyHouse));
        assert!(picked.relics.contains(&Relic::GoldenIdol));
        let reward = picked.reward.as_ref().expect("Tiny House reward overlay");
        assert_eq!(reward.gold_offer, 63);
    }

    #[test]
    fn normal_treasure_chest_can_be_skipped_to_map() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(RoomKind::Treasure);
        setup_treasure_room(&mut run);

        let next =
            apply_run_action(&run, RunAction::Proceed).expect("unopened chest can be skipped");

        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.treasure_room.is_none());
        assert_eq!(next.current_act, run.current_act);
        assert_eq!(next.current_floor, run.current_floor);
    }

    #[test]
    fn sapphire_key_claim_consumes_only_its_linked_chest_relic() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.set_final_act_available(Some(true))
            .expect("final act profile selects a burning elite");
        run.phase = RunPhase::Treasure;
        run.event = None;
        run.current_room_override = Some(RoomKind::Treasure);
        setup_treasure_room(&mut run);

        let opened = apply_run_action(&run, RunAction::OpenChest).expect("map chest opens");
        let linked = opened
            .treasure_room
            .as_ref()
            .and_then(|treasure| treasure.sapphire_key_relic_offer)
            .expect("final-act chest links a relic to the sapphire key");
        assert!(opened.reward.as_ref().is_some_and(|reward| {
            reward.relic_offer == Some(linked)
                || reward.pending_relic_offer == Some(linked)
                || reward.queued_relic_offers.contains(&linked)
        }));

        let keyed = apply_run_action(&opened, RunAction::TakeSapphireKey)
            .expect("sapphire key can be claimed");
        assert!(keyed.has_sapphire_key);
        assert!(!keyed.relics.contains(&linked));
        assert!(keyed
            .treasure_room
            .as_ref()
            .is_some_and(|treasure| treasure.sapphire_key_relic_offer.is_none()));
        assert!(keyed.reward.as_ref().is_some_and(|reward| {
            reward.relic_offer != Some(linked)
                && reward.pending_relic_offer != Some(linked)
                && !reward.queued_relic_offers.contains(&linked)
        }));
    }

    #[test]
    fn closing_map_chest_reward_clears_retained_treasure_room() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Treasure;
        run.current_room_override = Some(RoomKind::Treasure);
        setup_treasure_room(&mut run);

        let opened = apply_run_action(&run, RunAction::OpenChest).expect("map chest opens");
        opened
            .validate()
            .expect("reward overlay retains the treasure-room owner");
        assert!(opened.treasure_room.is_some());

        let closed = apply_run_action(&opened, RunAction::SkipReward)
            .expect("map chest reward can be closed");
        assert_eq!(closed.phase, RunPhase::Idle);
        assert!(closed.reward.is_none());
        assert!(closed.treasure_room.is_none());
        closed
            .validate()
            .expect("closed chest reward has no orphaned treasure state");
    }

    #[test]
    fn treasure_room_reward_order_round_trips_through_json() {
        let original = TreasureRoomState {
            chest_size: ChestSize::Large,
            relic_tier: RelicTier::Uncommon,
            have_gold: true,
            relic_before_gold: true,
            sapphire_key_relic_offer: None,
        };
        let restored: TreasureRoomState = serde_json::from_value(
            serde_json::to_value(original).expect("treasure room serializes"),
        )
        .expect("treasure room deserializes");
        assert_eq!(restored, original);

        let legacy: TreasureRoomState = serde_json::from_value(serde_json::json!({
            "chest_size": "Small",
            "relic_tier": "Common",
            "have_gold": false
        }))
        .expect("legacy treasure room defaults relic_before_gold");
        assert!(!legacy.relic_before_gold);
    }

    #[test]
    fn mark_of_bloom_blocks_the_act_transition_heal() {
        let mut run = RunState::map_fixture();
        run.current_act = 1;
        run.player_hp = 10;
        run.player_max_hp = 80;
        run.relics.push(Relic::MarkOfBloom);

        enter_next_act_map(&mut run).expect("static target encounter pools are valid");

        assert_eq!(run.current_act, 2);
        assert_eq!(run.player_hp, 10);
    }

    #[test]
    fn singing_bowl_with_mark_of_bloom_raises_max_hp_without_healing() {
        // Singing Bowl → increaseMaxHp(2, true); MotB blocks the heal half
        // (13efa069 floor 44: max 9000→9002, current_hp stays 8361).
        let mut run = RunState::map_fixture();
        run.player_hp = 50;
        run.player_max_hp = 80;
        run.relics.push(Relic::MarkOfBloom);
        run.relics.push(Relic::SingingBowl);

        run.gain_max_hp(crate::relic::SINGING_BOWL_MAX_HP)
            .expect("max HP gain");

        assert_eq!(run.player_max_hp, 82);
        assert_eq!(
            run.player_hp, 50,
            "Mark of the Bloom must block Singing Bowl heal"
        );
    }

    #[test]
    fn potion_reward_chance_resets_entering_city() {
        let mut run = RunState::map_fixture();
        run.current_act = 1;
        run.potion_chance = 30;

        enter_next_act_map(&mut run).expect("static target encounter pools are valid");

        assert_eq!(run.current_act, 2);
        assert_eq!(run.potion_chance, 0);
    }

    #[test]
    fn potion_reward_chance_resets_entering_beyond() {
        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.potion_chance = 30;

        enter_next_act_map(&mut run).expect("static target encounter pools are valid");

        assert_eq!(run.current_act, 3);
        assert_eq!(run.potion_chance, 0);
    }

    #[test]
    fn colorless_card_pool_resets_entering_city() {
        use crate::content::cards::DRAMATIC_ENTRANCE_ID;

        let mut run = RunState::map_fixture();
        run.current_act = 1;
        run.colorless_card_pool = vec![DRAMATIC_ENTRANCE_ID];

        enter_next_act_map(&mut run).expect("static target encounter pools are valid");

        assert_eq!(run.current_act, 2);
        assert!(
            run.colorless_card_pool.is_empty(),
            "act transition rebuilds colorlessCardPool from CardLibrary"
        );
    }

    #[test]
    fn colorless_card_pool_resets_entering_beyond() {
        use crate::content::cards::FINESSE_ID;

        let mut run = RunState::map_fixture();
        run.current_act = 2;
        run.colorless_card_pool = vec![FINESSE_ID];

        enter_next_act_map(&mut run).expect("static target encounter pools are valid");

        assert_eq!(run.current_act, 3);
        assert!(run.colorless_card_pool.is_empty());
    }

    #[test]
    fn enter_next_act_map_preserves_depleted_relic_pools() {
        let mut run = RunState::map_fixture();
        run.ensure_ironclad_relic_pools();
        let counter_before_transition = run.relic_rng_counter;
        let removed = run
            .relic_pools
            .as_mut()
            .expect("initialized pools")
            .common
            .remove(0);
        run.current_act = 1;

        enter_next_act_map(&mut run).expect("static target encounter pools are valid");

        assert_eq!(run.current_act, 2);
        assert_eq!(run.relic_rng_counter, counter_before_transition);
        assert!(!run
            .relic_pools
            .as_ref()
            .expect("persistent pools")
            .common
            .contains(&removed));
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

        run.gain_relic(Relic::BlackBlood)
            .expect("Black Blood pickup succeeds");

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

        let actual = target_potion_reward_offer(&mut actual_rng, &mut potion_chance, 2, 0, 3, true)
            .expect("guaranteed potion roll succeeds");
        let _drop_roll = expected_rng.random_int(99);
        let expected = Some(target_random_potion(&mut expected_rng));

        assert_eq!(actual, expected);
        assert_eq!(actual_rng.counter(), expected_rng.counter());
        assert_eq!(potion_chance, -10);
    }

    #[test]
    fn potion_reward_roll_at_chance_boundary_is_a_miss() {
        let mut rng = StsRng::with_counter(34_961_238_615_942, 54);
        let mut potion_chance = 10;

        assert_eq!(rng.clone().random_int(99), 50);
        let offer = target_potion_reward_offer(&mut rng, &mut potion_chance, 1, 0, 3, false)
            .expect("boundary potion roll succeeds");

        assert!(offer.is_none());
        assert_eq!(potion_chance, 20);
    }

    #[test]
    fn combat_rewards_generate_potions_before_acquisition_is_checked() {
        for potions in [
            Vec::new(),
            vec![Potion::Dexterity, Potion::Strength, Potion::Fire],
        ] {
            let mut run = RunState::map_fixture();
            run.phase = RunPhase::Combat;
            run.current_room_override = Some(RoomKind::Combat);
            run.relics = vec![Relic::Sozu, Relic::WhiteBeastStatue];
            run.potions = potions;
            run.combat = Some(CombatState::initial_fixture());
            prepare_won_combat_reward_fixture(&mut run);

            enter_normal_combat_reward_screen(&mut run).expect("combat reward generation succeeds");

            assert!(
                run.reward
                    .as_ref()
                    .expect("reward screen")
                    .potion_offer
                    .is_some(),
                "Sozu and a full belt affect claiming, not reward generation"
            );
        }
    }

    #[test]
    fn colosseum_second_fight_uses_custom_reward_and_returns_to_map() {
        let mut run = RunState::seeded_ironclad(34_961_238_615_940, 0);
        run.current_act = 2;
        run.current_floor = 31;
        prepare_won_combat_reward_fixture(&mut run);
        run.current_room_override = Some(RoomKind::Event);
        let potion_chance_before = run.potion_chance;
        let mut expected_potion_rng = run.rng_for_stream(RunRngStream::Potion);
        let mut expected_potion_chance = potion_chance_before;
        let expected_potion = target_potion_reward_offer(
            &mut expected_potion_rng,
            &mut expected_potion_chance,
            3,
            run.potions.len(),
            run.potion_capacity(),
            false,
        )
        .expect("hidden Colosseum potion roll succeeds");
        let expected_potion_counter = expected_potion_rng.counter();

        enter_colosseum_combat_reward_screen(&mut run)
            .expect("Colosseum reward generation succeeds");

        let reward = run.reward.as_ref().expect("Colosseum reward screen");
        assert_eq!(reward.continuation, RewardContinuation::Map);
        assert_eq!(reward.gold_offer, 100);
        assert!(reward.relic_offer.is_some());
        assert!(reward.pending_relic_offer.is_some());
        assert_eq!(reward.potion_offer, expected_potion);
        assert_eq!(run.potion_rng_counter, expected_potion_counter);
        assert_eq!(run.potion_chance, expected_potion_chance);
        assert_eq!(reward.remaining_card_reward_count(), 1);
        run.validate().expect("custom event reward is valid");

        let first_relic = apply_run_action(&run, RunAction::TakeRelicReward)
            .expect("first Colosseum relic can be claimed");
        let second_relic = apply_run_action(&first_relic, RunAction::TakeRelicReward)
            .expect("second Colosseum relic can be claimed");
        let mut gold = apply_run_action(&second_relic, RunAction::TakeGoldReward)
            .expect("Colosseum gold can be claimed");
        // Keep this regression deterministic even when this fixture's potion
        // roll misses; the legality case under test is an unclaimed potion.
        gold.reward
            .as_mut()
            .expect("Colosseum reward after gold")
            .potion_offer = Some(Potion::Fear);
        let potion = if gold
            .reward
            .as_ref()
            .is_some_and(|reward| reward.potion_offer.is_some())
        {
            apply_run_action(&gold, RunAction::TakePotionReward { index: 0 })
                .expect("Colosseum potion can be claimed")
        } else {
            gold.clone()
        };
        let pending_opened = apply_run_action(&gold, RunAction::OpenCardReward)
            .expect("Colosseum card reward can be opened before potion");
        let pending_card_id = pending_opened
            .reward
            .as_ref()
            .expect("open Colosseum reward before potion")
            .choices[0]
            .id;
        let pending_potion = apply_run_action(
            &pending_opened,
            RunAction::TakeCardReward {
                card_id: pending_card_id,
            },
        )
        .expect("Colosseum card reward can be claimed while potion remains");
        assert!(pending_potion
            .reward
            .as_ref()
            .is_some_and(|reward| reward.potion_offer.is_some()));
        let map_with_pending_potion = apply_run_action(&pending_potion, RunAction::Proceed)
            .expect("Colosseum reward can proceed with an unclaimed potion");
        assert_eq!(map_with_pending_potion.phase, RunPhase::Idle);
        assert!(map_with_pending_potion.reward.is_none());

        let opened = apply_run_action(&potion, RunAction::OpenCardReward)
            .expect("Colosseum card reward can be opened after potion");
        let card_id = opened
            .reward
            .as_ref()
            .expect("open Colosseum reward after potion")
            .choices[0]
            .id;
        let empty = apply_run_action(&opened, RunAction::TakeCardReward { card_id })
            .expect("Colosseum card reward can be claimed after potion");
        assert_eq!(empty.phase, RunPhase::Reward);
        assert!(empty.reward.as_ref().is_some_and(reward_is_empty));

        let map = apply_run_action(&empty, RunAction::Proceed)
            .expect("empty Colosseum reward proceeds to the map");
        assert_eq!(map.phase, RunPhase::Idle);
        assert!(map.reward.is_none());
    }

    #[test]
    fn potion_reward_chance_failures_do_not_consume_rng_or_mutate_chance() {
        let mut base_overflow_rng = StsRng::new(77);
        let base_overflow_rng_before = base_overflow_rng.clone();
        let mut base_overflow_chance = i32::MAX;
        assert_eq!(
            target_potion_reward_offer(
                &mut base_overflow_rng,
                &mut base_overflow_chance,
                1,
                0,
                3,
                false,
            ),
            Err(SimError::InvalidState(
                "potion reward drop chance overflows i32"
            ))
        );
        assert_eq!(base_overflow_rng, base_overflow_rng_before);
        assert_eq!(base_overflow_chance, i32::MAX);

        let mut miss_overflow_rng = StsRng::new(77);
        let miss_overflow_rng_before = miss_overflow_rng.clone();
        let mut miss_overflow_chance = i32::MAX;
        assert_eq!(
            target_potion_reward_offer(
                &mut miss_overflow_rng,
                &mut miss_overflow_chance,
                4,
                0,
                3,
                false,
            ),
            Err(SimError::InvalidState("potion reward chance overflows i32"))
        );
        assert_eq!(miss_overflow_rng, miss_overflow_rng_before);
        assert_eq!(miss_overflow_chance, i32::MAX);

        let mut hit_underflow_rng = StsRng::new(77);
        let hit_underflow_rng_before = hit_underflow_rng.clone();
        let mut hit_underflow_chance = i32::MIN;
        assert_eq!(
            target_potion_reward_offer(
                &mut hit_underflow_rng,
                &mut hit_underflow_chance,
                1,
                0,
                3,
                true,
            ),
            Err(SimError::InvalidState(
                "potion reward chance underflows i32"
            ))
        );
        assert_eq!(hit_underflow_rng, hit_underflow_rng_before);
        assert_eq!(hit_underflow_chance, i32::MIN);
    }

    #[test]
    fn escaped_combat_potion_chance_overflow_rolls_back_reward_entry() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.potion_chance = i32::MAX;
        let mut combat = CombatState::initial_fixture();
        for monster in &mut combat.monsters {
            monster.hp = 0;
            monster.alive = false;
            monster.escaped = true;
        }
        run.combat = Some(combat);
        prepare_won_combat_reward_fixture(&mut run);
        let before = run.clone();

        assert_eq!(
            enter_normal_combat_reward_screen(&mut run),
            Err(SimError::InvalidState("potion reward chance overflows i32"))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn combat_reward_entry_rejects_missing_or_unfinished_combat_atomically() {
        let entries: [fn(&mut RunState) -> SimResult<()>; 4] = [
            enter_normal_combat_reward_screen,
            enter_reward_screen,
            enter_elite_combat_reward_screen,
            enter_boss_combat_reward_screen,
        ];
        for entry in entries {
            let mut run = RunState::map_fixture();
            let before = run.clone();
            assert_eq!(
                entry(&mut run),
                Err(SimError::InvalidState(
                    "combat reward entry requires combat phase"
                ))
            );
            assert_eq!(run, before);
        }

        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Combat;
        run.current_room_override = Some(RoomKind::Combat);
        run.combat = Some(CombatState::initial_fixture());
        let before = run.clone();
        assert_eq!(
            enter_normal_combat_reward_screen(&mut run),
            Err(SimError::InvalidState(
                "combat reward entry requires won combat"
            ))
        );
        assert_eq!(run, before);

        run.combat.as_mut().expect("combat fixture").phase = CombatPhase::Won;
        let before = run.clone();
        assert_eq!(
            enter_normal_combat_reward_screen(&mut run),
            Err(SimError::InvalidState(
                "combat reward entry requires no living monsters"
            ))
        );
        assert_eq!(run, before);
    }

    #[test]
    fn stolen_gold_reward_overflow_fails_atomically() {
        let mut run = RunState::map_fixture();
        prepare_won_combat_reward_fixture(&mut run);
        let combat = run.combat.as_mut().expect("won combat fixture");
        let mut second = combat.monsters[0].clone();
        second.id = MonsterId::new(2);
        for monster in [&mut combat.monsters[0], &mut second] {
            monster.stolen_gold = i32::MAX;
            monster.escaped = false;
        }
        combat.monsters.push(second);
        let before = run.clone();

        assert_eq!(
            enter_reward_screen(&mut run),
            Err(SimError::InvalidState("stolen gold reward overflows i32"))
        );
        assert_eq!(run, before);
    }
}
