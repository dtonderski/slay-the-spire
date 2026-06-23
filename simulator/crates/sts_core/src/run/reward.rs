use crate::{
    card::{CardInstance, CardRarity},
    combat::{apply_combat_action, CombatPhase},
    content::cards::{upgrade_content_id, ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID},
    content::reward_pool::{ironclad_reward_card_rarity, RewardCardEntry, IRONCLAD_REWARD_ENTRIES},
    ids::CardId,
    potion::{Potion, PotionRarity, IRONCLAD_POTION_POOL, MAX_POTIONS},
    relic::{Relic, RelicKey, RelicTier},
    rng::{RngStream, SimulatorRng, StsRng},
    run::potion::{
        apply_combat_card_reward_choice, apply_discard_select_choice, apply_discard_select_confirm,
        apply_exhaust_select_choice, apply_exhaust_select_confirm, apply_hand_select_choice,
        apply_hand_select_confirm, apply_potion_action,
    },
    run::shop::apply_shop_action,
    CombatAction, ContentId, RewardScreen, RunAction, RunPhase, RunState, SimError, SimResult,
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
const SMALL_CHEST_CHANCE: i32 = 50;
const MEDIUM_CHEST_CHANCE: i32 = 33;
const CHEST_GOLD_CHANCES: [i32; 3] = [50, 35, 50];
const CHEST_RELIC_COMMON_CHANCES: [i32; 3] = [75, 35, 0];
const CHEST_RELIC_UNCOMMON_CHANCES: [i32; 3] = [25, 50, 75];

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

pub fn setup_treasure_room(run: &mut RunState) {
    let mut treasure_rng =
        StsRng::with_counter(run.treasure_rng_seed as i64, run.treasure_rng_counter);
    let chest_size = target_chest_size(&mut treasure_rng);
    let roll = treasure_rng.random_int(99);
    let have_gold = roll
        < CHEST_GOLD_CHANCES[match chest_size {
            ChestSize::Small => 0,
            ChestSize::Medium => 1,
            ChestSize::Large => 2,
        }];
    let relic_tier = target_chest_relic_tier(chest_size, roll);
    run.treasure_rng_counter = treasure_rng.counter();
    run.treasure_room = Some(TreasureRoomState {
        chest_size,
        relic_tier,
        have_gold,
    });
}

pub fn roll_event_relic_reward(run: &mut RunState, act: i32) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let mut relic_rng = StsRng::with_counter(run.relic_rng_seed as i64, run.relic_rng_counter);
    let tier = target_relic_tier(&mut relic_rng, act);
    run.relic_rng_counter = relic_rng.counter();
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

/// Deterministic fixed pool used in early milestones before RNG wiring.
#[must_use]
pub fn fixed_card_reward_choices(next_card_id: u64) -> Vec<CardInstance> {
    [ANGER_ID, CLEAVE_ID, SHRUG_IT_OFF_ID]
        .iter()
        .enumerate()
        .map(|(index, content_id)| {
            CardInstance::new(CardId::new(next_card_id + index as u64), *content_id)
        })
        .collect()
}

fn roll_reward_rarity(rng: &mut StsRng, card_rarity_factor: i32) -> CardRarity {
    let roll = rng.random_int(99) + card_rarity_factor;
    if roll < 3 {
        CardRarity::Rare
    } else if roll < 40 {
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
pub fn card_reward_choices(rng: &mut SimulatorRng, next_card_id: u64) -> Vec<CardInstance> {
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

#[must_use]
pub fn target_card_reward_choices(
    rng: &mut StsRng,
    card_rarity_factor: &mut i32,
    next_card_id: u64,
) -> Vec<CardInstance> {
    let mut choices = Vec::with_capacity(REWARD_CARD_COUNT);

    for index in 0..REWARD_CARD_COUNT {
        let requested = roll_reward_rarity(rng, *card_rarity_factor);
        let rarity = resolve_rarity(requested, IRONCLAD_REWARD_ENTRIES);
        match requested {
            CardRarity::Common => *card_rarity_factor = (*card_rarity_factor - 1).max(-40),
            CardRarity::Rare => *card_rarity_factor = 5,
            CardRarity::Uncommon => {}
        }

        let mut content_id;
        loop {
            let candidate_indices: Vec<usize> = IRONCLAD_REWARD_ENTRIES
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.rarity == rarity)
                .map(|(index, _)| index)
                .collect();
            let pick = rng.random_int((candidate_indices.len() - 1) as i32) as usize;
            content_id = IRONCLAD_REWARD_ENTRIES[candidate_indices[pick]].content_id;
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

pub fn target_normal_combat_gold(rng: &mut StsRng) -> i32 {
    rng.random_int_range(NORMAL_COMBAT_GOLD_MIN, NORMAL_COMBAT_GOLD_MAX)
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
    let rarity = match rng.random_int_range(0, 99) {
        roll if roll < 65 => PotionRarity::Common,
        roll if roll < 90 => PotionRarity::Uncommon,
        _ => PotionRarity::Rare,
    };

    loop {
        let index = rng.random_int((IRONCLAD_POTION_POOL.len() - 1) as i32) as usize;
        let potion = IRONCLAD_POTION_POOL[index];
        if potion.rarity() == rarity {
            return potion;
        }
    }
}

pub fn target_potion_reward_offer(
    rng: &mut StsRng,
    potion_chance: &mut i32,
    reward_count: usize,
    potion_belt_count: usize,
) -> Option<Potion> {
    if potion_belt_count >= MAX_POTIONS {
        return None;
    }

    let mut chance = BASE_POTION_DROP_CHANCE + *potion_chance;
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

fn roll_relic_reward(run: &mut RunState, tier: RelicTier) -> RelicKey {
    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, false);
    let pools = run.relic_pools.as_mut().expect("relic pools initialized");
    pools.return_random_relic(tier, &context)
}

pub fn enter_relic_reward_screen(run: &mut RunState, kind: CombatRewardKind) {
    run.ensure_ironclad_relic_pools();
    let mut relic_rng = StsRng::with_counter(run.relic_rng_seed as i64, run.relic_rng_counter);
    let tier = match kind {
        CombatRewardKind::Elite => target_elite_relic_tier(&mut relic_rng),
        CombatRewardKind::Chest | CombatRewardKind::Boss => {
            target_relic_tier(&mut relic_rng, run.current_act)
        }
        CombatRewardKind::Normal => unreachable!("normal combat rewards do not offer relics"),
    };
    run.relic_rng_counter = relic_rng.counter();

    let key = roll_relic_reward(run, tier);
    let relic_offer = Relic::from_key(key);

    let mut potion_rng = StsRng::with_counter(run.potion_rng_seed as i64, run.potion_rng_counter);
    let _elite_potion_roll = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        2,
        run.potions.len(),
    );
    run.potion_rng_counter = potion_rng.counter();

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices: Vec::new(),
        gold_offer: 0,
        potion_offer: None,
        relic_offer,
        relic_key_offer: if relic_offer.is_some() {
            None
        } else {
            Some(key)
        },
        card_reward_active: false,
        card_reward_pending: false,
    });
}

pub fn enter_boss_relic_reward_screen(run: &mut RunState) {
    let key = roll_relic_reward(run, RelicTier::Boss);

    let relic_offer = Relic::from_key(key);
    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices: Vec::new(),
        gold_offer: 0,
        potion_offer: None,
        relic_offer,
        relic_key_offer: if relic_offer.is_some() {
            None
        } else {
            Some(key)
        },
        card_reward_active: false,
        card_reward_pending: false,
    });
}

/// Target-style combat entry advances `cardRng` three times before the next reward card roll.
pub fn advance_card_rng_for_combat_entry(run: &mut RunState) {
    let mut card_rng = StsRng::with_counter(run.reward_rng_seed as i64, run.card_rng_counter);
    for _ in 0..3 {
        let _ = card_rng.random_int(99);
    }
    run.card_rng_counter = card_rng.counter();
}

pub(crate) fn roll_pending_card_reward_choices(run: &mut RunState) {
    let next_card_id = run.next_card_instance_id();
    let mut card_rng = StsRng::with_counter(run.reward_rng_seed as i64, run.card_rng_counter);
    let mut choices =
        target_card_reward_choices(&mut card_rng, &mut run.card_rarity_factor, next_card_id);
    consume_reward_card_upgrade_rolls(&mut card_rng, &mut choices);
    run.card_rng_counter = card_rng.counter();
    if run.relic_keys.iter().any(|key| *key == RelicKey::ToxicEgg) {
        for choice in &mut choices {
            if reward_card_is_skill(choice.content_id) {
                if let Some(upgraded) = upgrade_content_id(choice.content_id) {
                    choice.content_id = upgraded;
                }
            }
        }
    }
    run.reward.as_mut().expect("reward screen present").choices = choices;
}

fn consume_reward_card_upgrade_rolls(rng: &mut StsRng, choices: &mut [CardInstance]) {
    for choice in choices {
        if ironclad_reward_card_rarity(choice.content_id) == Some(CardRarity::Rare) {
            continue;
        }

        let upgrades = rng.random_float() < 0.0;
        if upgrades {
            if let Some(upgraded) = upgrade_content_id(choice.content_id) {
                choice.content_id = upgraded;
            }
        }
    }
}

fn reward_card_is_skill(content_id: ContentId) -> bool {
    use crate::content::cards::{
        ARMAMENTS_ID, BATTLE_TRANCE_ID, DEFEND_R_ID, ENTRENCH_ID, FLEX_ID, INTIMIDATE_ID,
        OFFERING_ID, SHOCKWAVE_ID, SHRUG_IT_OFF_ID, SPOT_WEAKNESS_ID, TRUE_GRIT_ID, WARCRY_ID,
        WARCRY_PLUS_ID,
    };
    matches!(
        content_id,
        DEFEND_R_ID
            | SHRUG_IT_OFF_ID
            | TRUE_GRIT_ID
            | BATTLE_TRANCE_ID
            | ENTRENCH_ID
            | FLEX_ID
            | INTIMIDATE_ID
            | OFFERING_ID
            | SHOCKWAVE_ID
            | SPOT_WEAKNESS_ID
            | WARCRY_ID
            | WARCRY_PLUS_ID
            | ARMAMENTS_ID
    )
}

pub fn enter_normal_combat_reward_screen(run: &mut RunState) {
    let mut treasure_rng =
        StsRng::with_counter(run.treasure_rng_seed as i64, run.treasure_rng_counter);
    let gold_offer = target_normal_combat_gold(&mut treasure_rng);
    run.treasure_rng_counter = treasure_rng.counter();

    let mut potion_rng = StsRng::with_counter(run.potion_rng_seed as i64, run.potion_rng_counter);
    let potion_offer = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        1,
        run.potions.len(),
    );
    run.potion_rng_counter = potion_rng.counter();

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices: Vec::new(),
        gold_offer,
        potion_offer,
        relic_offer: None,
        relic_key_offer: None,
        card_reward_active: false,
        card_reward_pending: true,
    });
}

pub fn enter_reward_screen(run: &mut RunState) {
    enter_normal_combat_reward_screen(run);
}

pub fn enter_elite_combat_reward_screen(run: &mut RunState) {
    let mut treasure_rng =
        StsRng::with_counter(run.treasure_rng_seed as i64, run.treasure_rng_counter);
    let gold_offer = target_normal_combat_gold(&mut treasure_rng);
    run.treasure_rng_counter = treasure_rng.counter();

    let mut relic_rng = StsRng::with_counter(run.relic_rng_seed as i64, run.relic_rng_counter);
    let tier = target_elite_relic_tier(&mut relic_rng);
    run.relic_rng_counter = relic_rng.counter();
    let key = roll_relic_reward(run, tier);
    let relic_offer = Relic::from_key(key);

    let mut potion_rng = StsRng::with_counter(run.potion_rng_seed as i64, run.potion_rng_counter);
    let _elite_potion_roll = target_potion_reward_offer(
        &mut potion_rng,
        &mut run.potion_chance,
        2,
        run.potions.len(),
    );
    run.potion_rng_counter = potion_rng.counter();

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices: Vec::new(),
        gold_offer,
        potion_offer: None,
        relic_offer,
        relic_key_offer: if relic_offer.is_some() {
            None
        } else {
            Some(key)
        },
        card_reward_active: false,
        card_reward_pending: true,
    });
}

pub fn enter_elite_relic_reward_screen(run: &mut RunState) {
    enter_relic_reward_screen(run, CombatRewardKind::Elite);
}

pub fn enter_chest_relic_reward_screen(run: &mut RunState) {
    if run.treasure_room.is_none() {
        setup_treasure_room(run);
    }
    let tier = run
        .treasure_room
        .as_ref()
        .expect("treasure room must be initialized before opening chest")
        .relic_tier;
    let key = roll_relic_reward(run, tier);
    let relic_offer = Relic::from_key(key);

    run.phase = RunPhase::Reward;
    run.combat = None;
    run.reward = Some(RewardScreen {
        choices: Vec::new(),
        gold_offer: 0,
        potion_offer: None,
        relic_offer,
        relic_key_offer: if relic_offer.is_some() {
            None
        } else {
            Some(key)
        },
        card_reward_active: false,
        card_reward_pending: false,
    });
}

pub fn apply_combat_action_on_run(run: &RunState, action: CombatAction) -> SimResult<RunState> {
    if run.phase != RunPhase::Combat {
        return Err(SimError::IllegalAction(
            "combat actions require combat phase",
        ));
    }

    let combat = run
        .combat
        .as_ref()
        .ok_or(SimError::InvalidState("combat state is missing"))?;

    let next_combat = apply_combat_action(combat, action)?;
    let mut next = run.clone();
    next.combat = Some(next_combat.clone());
    next.player_hp = next_combat.player.hp;
    next.player_max_hp = next_combat.player.max_hp;

    if next_combat.phase == CombatPhase::Won {
        enter_reward_screen(&mut next);
    }

    Ok(next)
}

pub fn apply_run_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    match action {
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
        RunAction::ChooseHandSelect { index } => apply_hand_select_choice(run, index),
        RunAction::ConfirmHandSelect => apply_hand_select_confirm(run),
        RunAction::ChooseDiscardSelect { index } => apply_discard_select_choice(run, index),
        RunAction::ConfirmDiscardSelect => apply_discard_select_confirm(run),
        RunAction::ChooseExhaustSelect { index } => apply_exhaust_select_choice(run, index),
        RunAction::ConfirmExhaustSelect => apply_exhaust_select_confirm(run),
        _ => apply_reward_action(run, action),
    }
}

fn apply_reward_action(run: &RunState, action: RunAction) -> SimResult<RunState> {
    run.validate_reward_action(action)?;

    let mut next = run.clone();
    match action {
        RunAction::SkipReward => {
            next.phase = RunPhase::Idle;
            next.reward = None;
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
            reward.card_reward_pending = false;
            next.deck.push(choice);
        }
        RunAction::TakeGoldReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            let gold_offer = reward.gold_offer;
            reward.gold_offer = 0;
            next.gold += gold_offer;
        }
        RunAction::TakePotionReward => {
            let potion = next
                .reward
                .as_mut()
                .expect("validated reward screen")
                .potion_offer
                .take()
                .expect("validated potion offer");
            next.potions.push(potion);
        }
        RunAction::TakeRelicReward => {
            let reward = next.reward.as_mut().expect("validated reward screen");
            if let Some(relic) = reward.relic_offer.take() {
                next.gain_relic(relic);
            } else if let Some(key) = reward.relic_key_offer.take() {
                next.gain_relic_key(key);
            }
        }
        RunAction::OpenCardReward => {
            if next
                .reward
                .as_ref()
                .is_some_and(|reward| reward.choices.is_empty() && reward.card_reward_pending)
            {
                roll_pending_card_reward_choices(&mut next);
            }
            next.reward
                .as_mut()
                .expect("validated reward screen")
                .card_reward_active = true;
        }
        RunAction::SkipPotionReward => {
            next.reward
                .as_mut()
                .expect("validated reward screen")
                .potion_offer = None;
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
        RunAction::ChooseCombatCardReward { .. } => {
            unreachable!("validated reward action")
        }
        RunAction::ChooseHandSelect { .. } | RunAction::ConfirmHandSelect => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        BASH_ID, BODY_SLAM_ID, CLEAVE_ID, CLOTHESLINE_ID, HAVOC_ID, SHRUG_IT_OFF_ID, STRIKE_R_ID,
        TWIN_STRIKE_ID,
    };
    use crate::relic::Relic;

    fn reward_pool_content_ids() -> Vec<crate::ContentId> {
        IRONCLAD_REWARD_ENTRIES
            .iter()
            .map(|entry| entry.content_id)
            .collect()
    }

    fn winning_combat_run() -> RunState {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat fixture");
        combat.monsters[0].hp = 14;

        let bash_id = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == BASH_ID)
            .expect("bash in hand")
            .id;
        let strike_id = combat
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == STRIKE_R_ID)
            .expect("strike in hand")
            .id;
        let monster_id = combat.monsters[0].id;

        run = apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: bash_id,
                target: Some(monster_id),
            },
        )
        .expect("bash applies");
        apply_combat_action_on_run(
            &run,
            CombatAction::PlayCard {
                card_id: strike_id,
                target: Some(monster_id),
            },
        )
        .expect("strike wins combat")
    }

    #[test]
    fn card_reward_choices_are_deterministic_for_seed() {
        let mut first = SimulatorRng::new(7);
        let mut second = SimulatorRng::new(7);

        assert_eq!(
            card_reward_choices(&mut first, 100),
            card_reward_choices(&mut second, 100)
        );
    }

    #[test]
    fn card_reward_choices_pick_three_unique_cards_from_pool() {
        let mut rng = SimulatorRng::new(42);
        let choices = card_reward_choices(&mut rng, 1);
        let pool = reward_pool_content_ids();

        assert_eq!(choices.len(), 3);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();
        assert_eq!(content_ids.len(), {
            let unique: std::collections::BTreeSet<_> = content_ids.iter().copied().collect();
            unique.len()
        });
        assert!(content_ids.iter().all(|id| pool.contains(id)));
    }

    #[test]
    fn card_reward_choices_use_separate_rarity_and_card_rng_streams() {
        let mut rng = SimulatorRng::new(11);
        let _ = card_reward_choices(&mut rng, 1);

        let streams: Vec<_> = rng.log().iter().map(|draw| draw.stream).collect();
        assert!(streams.contains(&RngStream::RewardRarity));
        assert!(streams.contains(&RngStream::RewardCard));
    }

    #[test]
    fn some_placeholder_seed_rolls_havoc_from_modeled_pool() {
        let havoc_found = (0_u64..10_000).any(|seed| {
            let mut rng = SimulatorRng::new(seed);
            card_reward_choices(&mut rng, 1)
                .iter()
                .any(|card| card.content_id == HAVOC_ID)
        });

        assert!(havoc_found);
    }

    #[test]
    fn placeholder_seed_7_reward_cards_match_golden_snapshot() {
        let mut rng = SimulatorRng::new(7);
        let choices = card_reward_choices(&mut rng, 100);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();

        assert_eq!(
            content_ids,
            vec![CLOTHESLINE_ID, SHRUG_IT_OFF_ID, CLEAVE_ID],
            "update snapshot if reward algorithm changes intentionally"
        );
    }

    #[test]
    fn target_card_reward_choices_use_sts_card_rng_and_rarity_factor() {
        let mut rng = StsRng::new(22_079_335_079);
        let mut card_rarity_factor = 5;

        let choices = target_card_reward_choices(&mut rng, &mut card_rarity_factor, 100);
        let content_ids: Vec<_> = choices.iter().map(|card| card.content_id).collect();

        assert_eq!(
            content_ids,
            vec![BODY_SLAM_ID, TWIN_STRIKE_ID, CLOTHESLINE_ID]
        );
        assert_eq!(rng.counter(), 6);
        assert_eq!(card_rarity_factor, 2);
    }

    #[test]
    fn combat_win_enters_reward_with_target_card_rng() {
        let mut run = winning_combat_run();

        run.reward_rng_seed = 22_079_335_079;
        run.card_rng_counter = 0;
        run.card_rarity_factor = 5;
        enter_reward_screen(&mut run);

        let reward = run.reward.as_ref().expect("reward screen present");
        assert!(reward.choices.is_empty());
        assert!(reward.card_reward_pending);

        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open cards");
        let reward = run.reward.expect("reward screen present");
        let content_ids: Vec<_> = reward.choices.iter().map(|card| card.content_id).collect();
        assert_eq!(
            content_ids,
            vec![BODY_SLAM_ID, TWIN_STRIKE_ID, CLOTHESLINE_ID]
        );
        assert_eq!(run.card_rarity_factor, 2);
        assert_eq!(run.card_rng_counter, 9);
    }

    #[test]
    fn target_card_reward_counter_persists_between_rewards() {
        let mut run = winning_combat_run();

        run.reward_rng_seed = 22_079_335_079;
        run.card_rng_counter = 0;
        run.card_rarity_factor = 5;
        enter_reward_screen(&mut run);
        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open first cards");
        let first_counter = run.card_rng_counter;
        let first_choices: Vec<_> = run
            .reward
            .as_ref()
            .expect("first reward")
            .choices
            .iter()
            .map(|card| card.content_id)
            .collect();

        advance_card_rng_for_combat_entry(&mut run);
        enter_reward_screen(&mut run);
        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open second cards");
        let second_choices: Vec<_> = run
            .reward
            .as_ref()
            .expect("second reward")
            .choices
            .iter()
            .map(|card| card.content_id)
            .collect();

        assert_eq!(first_counter, 9);
        assert!(run.card_rng_counter > first_counter);
        assert_ne!(second_choices, first_choices);
    }

    #[test]
    fn combat_win_enters_reward_with_three_rng_choices() {
        let mut run = winning_combat_run();
        let pool = reward_pool_content_ids();

        assert_eq!(run.phase, RunPhase::Reward);
        assert!(run.combat.is_none());
        let reward = run.reward.as_ref().expect("reward screen present");
        assert!(reward.choices.is_empty());
        assert!(reward.card_reward_pending);
        assert_eq!(reward.gold_offer, 11);
        assert_eq!(reward.potion_offer, None);
        assert_eq!(run.potion_chance, 10);
        assert_eq!(run.potion_rng_counter, 1);
        assert_eq!(reward.relic_offer, None);

        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open cards");
        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.choices.len(), 3);
        assert!(reward
            .choices
            .iter()
            .all(|card| pool.contains(&card.content_id)));
    }

    #[test]
    fn skip_reward_leaves_deck_unchanged() {
        let run = winning_combat_run();
        let deck_before = run.deck.clone();

        let next = apply_run_action(&run, RunAction::SkipReward).expect("skip reward");

        assert_eq!(next.phase, RunPhase::Idle);
        assert!(next.reward.is_none());
        assert_eq!(next.deck, deck_before);
    }

    #[test]
    fn take_card_reward_adds_choice_to_master_deck_and_stays_on_reward_screen() {
        let run =
            apply_run_action(&winning_combat_run(), RunAction::OpenCardReward).expect("open cards");
        let deck_len_before = run.deck.len();
        let chosen = run.reward.as_ref().expect("reward screen").choices[0].id;
        let chosen_content = run.reward.as_ref().expect("reward screen").choices[0].content_id;

        let next = apply_run_action(&run, RunAction::TakeCardReward { card_id: chosen })
            .expect("take reward");

        assert_eq!(next.phase, RunPhase::Reward);
        assert!(next.reward.as_ref().expect("reward").choices.is_empty());
        assert_eq!(next.deck.len(), deck_len_before + 1);
        assert!(next.deck.iter().any(|card| card.id == chosen));
        assert_eq!(next.count_content_in_deck(chosen_content), 1);
    }

    #[test]
    fn take_card_reward_rejects_unknown_card_id() {
        let run =
            apply_run_action(&winning_combat_run(), RunAction::OpenCardReward).expect("open cards");

        let err = apply_run_action(
            &run,
            RunAction::TakeCardReward {
                card_id: CardId::new(999),
            },
        )
        .expect_err("unknown reward card");

        assert_eq!(err, SimError::UnknownCard(CardId::new(999)));
    }

    #[test]
    fn take_gold_reward_adds_fixed_amount_and_leaves_deck_unchanged() {
        let run = winning_combat_run();
        let deck_before = run.deck.clone();
        let gold_before = run.gold;

        let next = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").gold_offer, 0);
        assert_eq!(next.deck, deck_before);
        assert_eq!(next.gold, gold_before + 11);
    }

    #[test]
    fn take_gold_reward_rejects_already_taken_gold() {
        let run = winning_combat_run();
        let next = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");

        let err = apply_run_action(&next, RunAction::TakeGoldReward).expect_err("gold taken");

        assert_eq!(err, SimError::IllegalAction("no gold reward offered"));
    }

    #[test]
    fn take_potion_reward_adds_fire_potion_to_belt() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").potion_offer = Some(Potion::Fire);
        let potions_before = run.potions.len();

        let next = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").potion_offer, None);
        assert_eq!(next.potions.len(), potions_before + 1);
        assert_eq!(next.potions.last(), Some(&Potion::Fire));
    }

    #[test]
    fn take_potion_reward_rejects_full_belt() {
        let mut run = winning_combat_run();
        run.potions = vec![Potion::Fire, Potion::Fire, Potion::Fire];
        run.reward.as_mut().expect("reward").potion_offer = Some(Potion::Fire);

        let err = apply_run_action(&run, RunAction::TakePotionReward).expect_err("belt full");

        assert_eq!(err, SimError::IllegalAction("potion belt is full"));
    }

    #[test]
    fn take_relic_reward_adds_oddly_smooth_stone() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").relic_offer = Some(Relic::OddlySmoothStone);

        let next = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").relic_offer, None);
        assert_eq!(next.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn take_relic_reward_accepts_implemented_relic_key_offer() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").relic_key_offer = Some(Relic::OddlySmoothStone.key());

        let next = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic key");

        assert_eq!(next.phase, RunPhase::Reward);
        assert_eq!(next.reward.as_ref().expect("reward").relic_key_offer, None);
        assert_eq!(next.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn take_relic_reward_accepts_unimplemented_relic_key_offer() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").relic_key_offer =
            Some(crate::RelicKey::ToyOrnithopter);

        let next = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic key");

        assert_eq!(next.relic_keys, vec![crate::RelicKey::ToyOrnithopter]);
        assert!(next.relics.is_empty());
    }

    #[test]
    fn multiple_reward_offers_can_be_taken_before_skip() {
        let mut run = winning_combat_run();
        run.reward.as_mut().expect("reward").potion_offer = Some(Potion::Fire);
        run.reward.as_mut().expect("reward").relic_offer = Some(Relic::OddlySmoothStone);

        let run = apply_run_action(&run, RunAction::TakeGoldReward).expect("take gold");
        let run = apply_run_action(&run, RunAction::TakePotionReward).expect("take potion");
        let run = apply_run_action(&run, RunAction::TakeRelicReward).expect("take relic");
        let run = apply_run_action(&run, RunAction::SkipReward).expect("leave reward");

        assert_eq!(run.phase, RunPhase::Idle);
        assert!(run.reward.is_none());
        assert_eq!(run.gold, crate::STARTING_GOLD + 11);
        assert_eq!(run.potions, vec![Potion::Fire]);
        assert_eq!(run.relics, vec![Relic::OddlySmoothStone]);
    }

    #[test]
    fn normal_combat_gold_uses_target_treasure_rng_range() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(target_normal_combat_gold(&mut rng), 19);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn target_relic_tier_uses_act_one_thresholds() {
        let mut uncommon_rng = StsRng::new(22_079_335_079);
        let mut common_rng = StsRng::new(22_079_335_079);
        common_rng.random_int(99);
        let mut rare_rng = StsRng::new(22_079_335_079);
        for _ in 0..10 {
            rare_rng.random_int(99);
        }

        assert_eq!(target_relic_tier(&mut common_rng, 1), RelicTier::Common);
        assert_eq!(target_relic_tier(&mut rare_rng, 1), RelicTier::Rare);
        assert_eq!(target_relic_tier(&mut uncommon_rng, 1), RelicTier::Uncommon);
    }

    #[test]
    fn target_relic_tier_uses_act_four_thresholds() {
        let mut rng = StsRng::new(22_079_335_079);

        assert_eq!(target_relic_tier(&mut rng, 4), RelicTier::Uncommon);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn target_elite_relic_tier_uses_target_thresholds() {
        let mut uncommon_rng = StsRng::new(22_079_335_079);
        let mut common_rng = StsRng::new(22_079_335_079);
        common_rng.random_int(99);
        let mut rare_rng = StsRng::new(22_079_335_079);
        for _ in 0..10 {
            rare_rng.random_int(99);
        }

        assert_eq!(target_elite_relic_tier(&mut common_rng), RelicTier::Common);
        assert_eq!(target_elite_relic_tier(&mut rare_rng), RelicTier::Rare);
        assert_eq!(
            target_elite_relic_tier(&mut uncommon_rng),
            RelicTier::Uncommon
        );
    }

    #[test]
    fn target_potion_reward_miss_increases_chance_and_consumes_drop_roll() {
        let mut rng = StsRng::new(0);
        let mut potion_chance = 0;

        let offer = target_potion_reward_offer(&mut rng, &mut potion_chance, 2, 0);

        assert_eq!(offer, None);
        assert_eq!(potion_chance, 10);
        assert_eq!(rng.counter(), 1);
    }

    #[test]
    fn target_potion_reward_hit_decreases_chance_and_rolls_pool() {
        let mut rng = StsRng::new(0);
        let mut potion_chance = 70;

        let offer = target_potion_reward_offer(&mut rng, &mut potion_chance, 2, 0);

        assert!(offer.is_some());
        assert_eq!(potion_chance, 60);
        assert!(rng.counter() > 1);
    }

    #[test]
    fn combat_win_persists_treasure_rng_counter() {
        let mut run = winning_combat_run();

        run.treasure_rng_seed = 22_079_335_079;
        run.treasure_rng_counter = 0;
        enter_reward_screen(&mut run);

        let reward = run.reward.expect("reward screen present");
        assert_eq!(reward.gold_offer, 19);
        assert_eq!(run.treasure_rng_counter, 1);
    }

    #[test]
    fn take_relic_reward_rejects_duplicate_relic() {
        let mut run = winning_combat_run();
        run.relics.push(Relic::OddlySmoothStone);
        run.reward.as_mut().expect("reward screen").relic_offer = Some(Relic::OddlySmoothStone);

        let err = apply_run_action(&run, RunAction::TakeRelicReward).expect_err("duplicate");

        assert_eq!(err, SimError::IllegalAction("relic already owned"));
    }

    #[test]
    fn codex03_reward_rng_counters_match_captured_trace_prefix() {
        use crate::content::cards::{
            ANGER_ID, HEADBUTT_ID, PERFECTED_STRIKE_ID, SWORD_BOOMERANG_ID, TRUE_GRIT_ID,
            UPPERCUT_ID, WHIRLWIND_ID,
        };
        use crate::RunAction;

        let seed = 22_079_335_078i64;
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = seed as u64;
        run.treasure_rng_seed = seed as u64;
        run.potion_rng_seed = seed as u64;
        run.card_rng_counter = 0;
        run.card_rarity_factor = 5;
        run.treasure_rng_counter = 0;
        run.potion_rng_counter = 0;
        run.potion_chance = 0;

        enter_normal_combat_reward_screen(&mut run);
        let reward = run.reward.as_ref().expect("floor-1 reward");
        assert_eq!(reward.gold_offer, 13);
        assert!(reward.choices.is_empty());

        run = apply_run_action(&run, RunAction::TakeGoldReward).expect("gold");
        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open cards");
        assert_eq!(
            run.reward
                .as_ref()
                .unwrap()
                .choices
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            vec![PERFECTED_STRIKE_ID, TRUE_GRIT_ID, HEADBUTT_ID]
        );
        run = apply_run_action(
            &run,
            RunAction::TakeCardReward {
                card_id: run.reward.as_ref().unwrap().choices[2].id,
            },
        )
        .expect("headbutt");

        advance_card_rng_for_combat_entry(&mut run);
        enter_normal_combat_reward_screen(&mut run);
        let reward = run.reward.as_ref().expect("floor-2 reward");
        assert_eq!(reward.gold_offer, 17);
        assert!(reward.choices.is_empty());

        run = apply_run_action(&run, RunAction::TakeGoldReward).expect("gold");
        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open cards");
        assert_eq!(
            run.reward
                .as_ref()
                .unwrap()
                .choices
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            vec![WHIRLWIND_ID, UPPERCUT_ID, PERFECTED_STRIKE_ID]
        );
        run = apply_run_action(
            &run,
            RunAction::TakeCardReward {
                card_id: run.reward.as_ref().unwrap().choices[1].id,
            },
        )
        .expect("uppercut");

        advance_card_rng_for_combat_entry(&mut run);
        enter_normal_combat_reward_screen(&mut run);
        let reward = run.reward.as_ref().expect("floor-3 reward");
        assert_eq!(reward.gold_offer, 13);
        assert!(reward.choices.is_empty());

        run = apply_run_action(&run, RunAction::OpenCardReward).expect("open cards");
        assert_eq!(
            run.reward
                .as_ref()
                .unwrap()
                .choices
                .iter()
                .map(|c| c.content_id)
                .collect::<Vec<_>>(),
            vec![SWORD_BOOMERANG_ID, ANGER_ID, TRUE_GRIT_ID]
        );
    }

    #[test]
    fn combat_fixture_starts_with_starting_gold() {
        let run = RunState::combat_fixture();

        assert_eq!(run.gold, crate::run::state::STARTING_GOLD);
    }

    #[test]
    fn codex04_floor1_reward_matches_captured_card_gold_and_potion_miss() {
        let mut run = winning_combat_run();
        run.reward_rng_seed = 22_079_335_079;
        run.treasure_rng_seed = 22_079_335_079;
        run.potion_rng_seed = 22_079_335_079;
        run.card_rng_counter = 3;
        run.card_rarity_factor = 5;
        run.treasure_rng_counter = 0;
        run.potion_rng_counter = 0;
        run.potion_chance = 0;
        run.current_floor = 1;

        enter_normal_combat_reward_screen(&mut run);

        let reward = run.reward.as_ref().expect("reward");
        assert_eq!(reward.gold_offer, 19);
        assert_eq!(reward.potion_offer, None);
        assert!(reward.choices.is_empty());

        let run = apply_run_action(&run, RunAction::OpenCardReward).expect("open cards");
        let content_ids: Vec<_> = run
            .reward
            .as_ref()
            .expect("reward")
            .choices
            .iter()
            .map(|c| c.content_id)
            .collect();
        assert_eq!(
            content_ids,
            vec![
                crate::content::cards::BATTLE_TRANCE_ID,
                crate::content::cards::TWIN_STRIKE_ID,
                crate::content::cards::ENTRENCH_ID,
            ]
        );
    }

    #[test]
    fn elite_relic_reward_pops_from_pool_with_elite_tier_roll() {
        let mut run = RunState::combat_fixture();
        run.relic_rng_seed = 22_079_335_079;
        run.current_floor = 5;
        run.ensure_ironclad_relic_pools();

        enter_elite_relic_reward_screen(&mut run);

        let reward = run.reward.expect("elite relic reward");
        assert!(reward.relic_offer.is_some() || reward.relic_key_offer.is_some());
        assert_eq!(reward.gold_offer, 0);
        assert!(reward.choices.is_empty());
    }

    #[test]
    fn elite_relic_reward_does_not_regress_relic_rng_counter_after_pool_init() {
        let mut run = RunState::combat_fixture();
        run.relic_rng_seed = 22_079_335_079;
        run.relic_rng_counter = 0;
        run.current_floor = 5;

        enter_elite_relic_reward_screen(&mut run);

        assert!(
            run.relic_rng_counter >= 5,
            "relic pool init should advance relic_rng_counter, got {}",
            run.relic_rng_counter
        );
    }
}
