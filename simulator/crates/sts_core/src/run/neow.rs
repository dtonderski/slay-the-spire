//! Neow reward and drawback helpers used by run simulation and seed-start verification.

use crate::{
    card::CardRarity,
    content::{
        reward_pool::{
            ironclad_transform_card_content_id, random_normal_curse, IRONCLAD_REWARD_ENTRIES,
        },
        shop_pool::random_colorless_from_pool,
    },
    ids::ContentId,
    potion::{Potion, IRONCLAD_POTION_POOL},
    relic::{Relic, RelicKey, RelicTier},
    rng::StsRng,
    run::state::{RewardScreen, RunPhase, RunRngStream, RunState},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeowRewardType {
    ThreeCards,
    OneRandomRareCard,
    RandomColorless,
    RandomColorlessTwo,
    RemoveCard,
    RemoveTwo,
    UpgradeCard,
    TransformCard,
    TransformTwoCards,
    ThreeSmallPotions,
    RandomCommonRelic,
    OneRareRelic,
    TenPercentHpBonus,
    TwentyPercentHpBonus,
    ThreeEnemyKill,
    HundredGold,
    TwoFiftyGold,
    BossRelic,
    ThreeRareCards,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeowDrawback {
    None,
    TenPercentHpLoss,
    NoGold,
    Curse,
    PercentDamage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedNeowOption {
    pub slot: usize,
    pub drawback: NeowDrawback,
    pub reward: NeowRewardType,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeowCardReward {
    pub cards: Vec<ContentId>,
    pub neow_rng_counter: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeowColorlessReward {
    pub cards: Vec<ContentId>,
    pub neow_rng_counter: u32,
    pub card_rng_counter: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeowPotionReward {
    pub potions: Vec<Potion>,
    pub potion_rng_counter: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeowBossSwapReward {
    pub relic: RelicKey,
    pub relic_rng_counter: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeowTransformReward {
    pub cards: Vec<ContentId>,
    pub neow_rng_counter: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeowRelicReward {
    pub relic: RelicKey,
    pub relic_rng_counter: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeowCurseDrawback {
    pub curse: ContentId,
    pub card_rng_counter: u32,
}

pub fn generate_neow_options(numeric_seed: i64, player_max_hp: i32) -> Vec<GeneratedNeowOption> {
    let mut rng = StsRng::new(numeric_seed);
    (0..4)
        .map(|slot| generate_neow_option(slot, player_max_hp, &mut rng))
        .collect()
}

pub fn generate_neow_options_rng_counter(numeric_seed: i64, player_max_hp: i32) -> u32 {
    let mut rng = StsRng::new(numeric_seed);
    for slot in 0..4 {
        generate_neow_option(slot, player_max_hp, &mut rng);
    }
    rng.counter()
}

pub fn generate_neow_card_reward(numeric_seed: i64, reward: NeowRewardType) -> NeowCardReward {
    let mut rng = StsRng::new(numeric_seed);
    for slot in 0..4 {
        generate_neow_option(slot, 80, &mut rng);
    }
    generate_neow_card_reward_with_rng(&mut rng, reward)
}

pub fn generate_neow_card_reward_with_rng(
    rng: &mut StsRng,
    reward: NeowRewardType,
) -> NeowCardReward {
    let cards = match reward {
        NeowRewardType::ThreeCards => neow_unique_ironclad_cards_with_rolled_rarity(rng, 3),
        NeowRewardType::OneRandomRareCard => vec![neow_random_ironclad_card(rng, CardRarity::Rare)],
        NeowRewardType::ThreeRareCards => {
            neow_unique_ironclad_cards_with_forced_rarity(rng, CardRarity::Rare, 3)
        }
        other => panic!("Neow reward {other:?} is not a card reward"),
    };

    NeowCardReward {
        cards,
        neow_rng_counter: rng.counter(),
    }
}

pub fn generate_neow_rare_card_reward(numeric_seed: i64, reward: NeowRewardType) -> NeowCardReward {
    generate_neow_card_reward(numeric_seed, reward)
}

pub fn generate_neow_rare_card_reward_with_rng(
    rng: &mut StsRng,
    reward: NeowRewardType,
) -> NeowCardReward {
    generate_neow_card_reward_with_rng(rng, reward)
}

pub fn generate_neow_colorless_reward(
    numeric_seed: i64,
    reward: NeowRewardType,
) -> NeowColorlessReward {
    let mut neow_rng = StsRng::new(numeric_seed);
    for slot in 0..4 {
        generate_neow_option(slot, 80, &mut neow_rng);
    }
    let mut card_rng = StsRng::new(numeric_seed);
    generate_neow_colorless_reward_with_rng(&mut neow_rng, &mut card_rng, reward)
}

pub fn generate_neow_colorless_reward_with_card_rng_counter(
    numeric_seed: i64,
    reward: NeowRewardType,
    card_rng_counter: u32,
) -> NeowColorlessReward {
    let mut neow_rng = StsRng::new(numeric_seed);
    for slot in 0..4 {
        generate_neow_option(slot, 80, &mut neow_rng);
    }
    let mut card_rng = StsRng::with_counter(numeric_seed, card_rng_counter);
    generate_neow_colorless_reward_with_rng(&mut neow_rng, &mut card_rng, reward)
}

pub fn generate_neow_colorless_reward_with_rng(
    neow_rng: &mut StsRng,
    card_rng: &mut StsRng,
    reward: NeowRewardType,
) -> NeowColorlessReward {
    let force_rare = match reward {
        NeowRewardType::RandomColorless => false,
        NeowRewardType::RandomColorlessTwo => true,
        other => panic!("Neow reward {other:?} is not a colorless reward"),
    };
    let cards = neow_unique_colorless_cards(neow_rng, card_rng, force_rare, 3);

    NeowColorlessReward {
        cards,
        neow_rng_counter: neow_rng.counter(),
        card_rng_counter: card_rng.counter(),
    }
}

pub fn generate_neow_three_potions(numeric_seed: i64) -> NeowPotionReward {
    let mut potion_rng = StsRng::new(numeric_seed);
    generate_neow_three_potions_with_rng(&mut potion_rng)
}

pub fn generate_neow_three_potions_with_rng(potion_rng: &mut StsRng) -> NeowPotionReward {
    let potions = (0..3).map(|_| neow_random_potion(potion_rng)).collect();

    NeowPotionReward {
        potions,
        potion_rng_counter: potion_rng.counter(),
    }
}

pub fn apply_neow_boss_swap(run: &mut RunState) -> NeowBossSwapReward {
    run.ensure_ironclad_relic_pools();

    run.relics.retain(|relic| *relic != Relic::BurningBlood);
    let context = run.relic_spawn_context(run.current_floor, false);
    let relic = run
        .relic_pools
        .as_mut()
        .expect("relic pools initialized")
        .return_random_relic(RelicTier::Boss, &context);
    if relic == RelicKey::TinyHouse && run.reward.is_none() {
        let continuation = if run
            .event
            .as_ref()
            .is_some_and(|event| event.event == crate::Event::Neow)
        {
            crate::RewardContinuation::Neow
        } else {
            crate::RewardContinuation::None
        };
        run.phase = RunPhase::Reward;
        run.reward = Some(RewardScreen {
            continuation,
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
    run.gain_relic_key(relic);

    NeowBossSwapReward {
        relic,
        relic_rng_counter: run.relic_rng_counter,
    }
}

pub fn generate_neow_transform_reward(
    numeric_seed: i64,
    sources: &[ContentId],
) -> NeowTransformReward {
    let mut rng = StsRng::new(numeric_seed);
    for slot in 0..4 {
        generate_neow_option(slot, 80, &mut rng);
    }
    generate_neow_transform_reward_with_rng(&mut rng, sources)
}

pub fn generate_neow_transform_reward_with_rng(
    rng: &mut StsRng,
    sources: &[ContentId],
) -> NeowTransformReward {
    let cards = sources
        .iter()
        .map(|source| ironclad_transform_card_content_id(*source, rng))
        .collect();

    NeowTransformReward {
        cards,
        neow_rng_counter: rng.counter(),
    }
}

pub fn apply_neow_relic_reward(run: &mut RunState, reward: NeowRewardType) -> NeowRelicReward {
    let tier = match reward {
        NeowRewardType::RandomCommonRelic => RelicTier::Common,
        NeowRewardType::OneRareRelic => RelicTier::Rare,
        other => panic!("Neow reward {other:?} is not a fixed-tier relic reward"),
    };

    run.ensure_ironclad_relic_pools();
    let context = run.relic_spawn_context(run.current_floor, false);
    let relic = run
        .relic_pools
        .as_mut()
        .expect("relic pools initialized")
        .return_random_relic(tier, &context);
    run.gain_relic_key(relic);

    NeowRelicReward {
        relic,
        relic_rng_counter: run.relic_rng_counter,
    }
}

pub fn apply_neow_simple_reward(run: &mut RunState, reward: NeowRewardType) {
    match reward {
        NeowRewardType::TenPercentHpBonus => gain_max_hp(run, ten_percent(run.player_max_hp)),
        NeowRewardType::TwentyPercentHpBonus => gain_max_hp(run, twenty_percent(run.player_max_hp)),
        NeowRewardType::HundredGold => run.gain_gold(100),
        NeowRewardType::TwoFiftyGold => run.gain_gold(250),
        other => panic!("Neow reward {other:?} is not a simple immediate reward"),
    }
}

pub fn apply_neow_lament_reward(run: &mut RunState) {
    run.neow_lament_combats_remaining = 3;
}

pub fn apply_neow_simple_drawback(run: &mut RunState, drawback: NeowDrawback) {
    match drawback {
        NeowDrawback::None => {}
        NeowDrawback::TenPercentHpLoss => lose_max_hp(run, ten_percent(run.player_max_hp)),
        NeowDrawback::NoGold => run.gold = 0,
        NeowDrawback::PercentDamage => {
            run.player_hp = (run.player_hp - percent_damage(run.player_max_hp)).max(1);
        }
        NeowDrawback::Curse => panic!("Neow curse drawback needs cardRng curse identity"),
    }
}

pub fn apply_neow_curse_drawback(run: &mut RunState) -> NeowCurseDrawback {
    let mut card_rng = run.rng_for_stream(RunRngStream::CardReward);
    let curse = neow_modeled_random_curse(&mut card_rng);
    run.store_rng_counter(RunRngStream::CardReward, &card_rng);
    run.gain_deck_card(curse);

    NeowCurseDrawback {
        curse,
        card_rng_counter: run.card_rng_counter,
    }
}

pub fn open_neow_reward_grid(run: &mut RunState, reward: NeowRewardType) {
    match reward {
        NeowRewardType::RemoveCard => super::grid::open_neow_remove_grid(run, 1),
        NeowRewardType::RemoveTwo => super::grid::open_neow_remove_grid(run, 2),
        NeowRewardType::UpgradeCard => super::grid::open_neow_upgrade_grid(run),
        NeowRewardType::TransformCard => super::grid::open_neow_transform_grid(run, 1),
        NeowRewardType::TransformTwoCards => super::grid::open_neow_transform_grid(run, 2),
        other => panic!("Neow reward {other:?} does not open a grid"),
    }
}

fn gain_max_hp(run: &mut RunState, amount: i32) {
    run.player_max_hp += amount;
    run.player_hp += amount;
}

fn lose_max_hp(run: &mut RunState, amount: i32) {
    run.player_max_hp = (run.player_max_hp - amount).max(1);
    run.player_hp = run.player_hp.min(run.player_max_hp);
}

fn generate_neow_option(slot: usize, player_max_hp: i32, rng: &mut StsRng) -> GeneratedNeowOption {
    let (drawback, rewards) = neow_reward_options(slot, rng);
    let reward_index = rng.random_int((rewards.len() - 1) as i32) as usize;
    let reward = rewards[reward_index];
    let label = format!(
        "{}{}",
        drawback_label(drawback, player_max_hp),
        reward_label(reward, player_max_hp)
    );

    GeneratedNeowOption {
        slot,
        drawback,
        reward,
        label,
    }
}

fn neow_reward_options(slot: usize, rng: &mut StsRng) -> (NeowDrawback, Vec<NeowRewardType>) {
    match slot {
        0 => (
            NeowDrawback::None,
            vec![
                NeowRewardType::ThreeCards,
                NeowRewardType::OneRandomRareCard,
                NeowRewardType::RemoveCard,
                NeowRewardType::UpgradeCard,
                NeowRewardType::TransformCard,
                NeowRewardType::RandomColorless,
            ],
        ),
        1 => (
            NeowDrawback::None,
            vec![
                NeowRewardType::ThreeSmallPotions,
                NeowRewardType::RandomCommonRelic,
                NeowRewardType::TenPercentHpBonus,
                NeowRewardType::ThreeEnemyKill,
                NeowRewardType::HundredGold,
            ],
        ),
        2 => {
            let drawback_options = [
                NeowDrawback::TenPercentHpLoss,
                NeowDrawback::NoGold,
                NeowDrawback::Curse,
                NeowDrawback::PercentDamage,
            ];
            let drawback = drawback_options[rng.random_int(3) as usize];
            let mut rewards = vec![NeowRewardType::RandomColorlessTwo];
            if drawback != NeowDrawback::Curse {
                rewards.push(NeowRewardType::RemoveTwo);
            }
            rewards.push(NeowRewardType::OneRareRelic);
            rewards.push(NeowRewardType::ThreeRareCards);
            if drawback != NeowDrawback::NoGold {
                rewards.push(NeowRewardType::TwoFiftyGold);
            }
            rewards.push(NeowRewardType::TransformTwoCards);
            if drawback != NeowDrawback::TenPercentHpLoss {
                rewards.push(NeowRewardType::TwentyPercentHpBonus);
            }
            (drawback, rewards)
        }
        3 => (NeowDrawback::None, vec![NeowRewardType::BossRelic]),
        _ => panic!("Neow option slot must be 0..=3"),
    }
}

fn drawback_label(drawback: NeowDrawback, player_max_hp: i32) -> String {
    match drawback {
        NeowDrawback::None => String::new(),
        NeowDrawback::TenPercentHpLoss => format!("lose {} max hp ", ten_percent(player_max_hp)),
        NeowDrawback::NoGold => "lose all gold ".to_owned(),
        NeowDrawback::Curse => "obtain a curse ".to_owned(),
        NeowDrawback::PercentDamage => format!("take {} damage ", percent_damage(player_max_hp)),
    }
}

fn reward_label(reward: NeowRewardType, player_max_hp: i32) -> String {
    match reward {
        NeowRewardType::ThreeCards => "choose a card to obtain".to_owned(),
        NeowRewardType::OneRandomRareCard => "obtain a random rare card".to_owned(),
        NeowRewardType::RandomColorless => "choose a colorless card to obtain".to_owned(),
        NeowRewardType::RandomColorlessTwo => "choose a rare colorless card to obtain".to_owned(),
        NeowRewardType::RemoveCard => "remove a card from your deck".to_owned(),
        NeowRewardType::RemoveTwo => "remove 2 cards".to_owned(),
        NeowRewardType::UpgradeCard => "upgrade a card".to_owned(),
        NeowRewardType::TransformCard => "transform a card".to_owned(),
        NeowRewardType::TransformTwoCards => "transform 2 cards".to_owned(),
        NeowRewardType::ThreeSmallPotions => "obtain 3 random potions".to_owned(),
        NeowRewardType::RandomCommonRelic => "obtain a random common relic".to_owned(),
        NeowRewardType::OneRareRelic => "obtain a random rare relic".to_owned(),
        NeowRewardType::TenPercentHpBonus => {
            format!("max hp +{}", ten_percent(player_max_hp))
        }
        NeowRewardType::TwentyPercentHpBonus => {
            format!("max hp +{}", twenty_percent(player_max_hp))
        }
        NeowRewardType::ThreeEnemyKill => "enemies in your next three combats have 1 hp".to_owned(),
        NeowRewardType::HundredGold => "obtain 100 gold".to_owned(),
        NeowRewardType::TwoFiftyGold => "gain 250 gold".to_owned(),
        NeowRewardType::BossRelic => {
            "lose your starting relic obtain a random boss relic".to_owned()
        }
        NeowRewardType::ThreeRareCards => "choose a rare card to obtain".to_owned(),
    }
}

fn ten_percent(player_max_hp: i32) -> i32 {
    player_max_hp / 10
}

fn twenty_percent(player_max_hp: i32) -> i32 {
    player_max_hp / 5
}

fn percent_damage(player_max_hp: i32) -> i32 {
    player_max_hp * 3 / 10
}

fn neow_random_ironclad_card(rng: &mut StsRng, rarity: CardRarity) -> ContentId {
    let pool: Vec<_> = IRONCLAD_REWARD_ENTRIES
        .iter()
        .filter(|entry| entry.rarity == rarity)
        .collect();
    assert!(!pool.is_empty(), "Neow card reward pool must not be empty");
    let pick = rng.random_int((pool.len() - 1) as i32) as usize;
    pool[pick].content_id
}

fn neow_unique_ironclad_cards_with_rolled_rarity(rng: &mut StsRng, count: usize) -> Vec<ContentId> {
    let mut cards = Vec::new();
    while cards.len() < count {
        let rarity = neow_normal_card_rarity(rng);
        loop {
            let candidate = neow_random_ironclad_card(rng, rarity);
            if !cards.contains(&candidate) {
                cards.push(candidate);
                break;
            }
        }
    }
    cards
}

fn neow_unique_ironclad_cards_with_forced_rarity(
    rng: &mut StsRng,
    rarity: CardRarity,
    count: usize,
) -> Vec<ContentId> {
    let mut cards = Vec::new();
    while cards.len() < count {
        let _rolled_rarity = neow_normal_card_rarity(rng);
        loop {
            let candidate = neow_random_ironclad_card(rng, rarity);
            if !cards.contains(&candidate) {
                cards.push(candidate);
                break;
            }
        }
    }
    cards
}

fn neow_normal_card_rarity(rng: &mut StsRng) -> CardRarity {
    if rng.random_float() < 0.33 {
        CardRarity::Uncommon
    } else {
        CardRarity::Common
    }
}

fn neow_unique_colorless_cards(
    neow_rng: &mut StsRng,
    card_rng: &mut StsRng,
    force_rare: bool,
    count: usize,
) -> Vec<ContentId> {
    let mut cards = Vec::new();
    while cards.len() < count {
        let rarity = neow_colorless_rarity(neow_rng, force_rare);
        loop {
            let candidate = random_colorless_from_pool(card_rng, rarity);
            if !cards.contains(&candidate) {
                cards.push(candidate);
                break;
            }
        }
    }
    cards
}

fn neow_colorless_rarity(neow_rng: &mut StsRng, force_rare: bool) -> CardRarity {
    let _rolled_uncommon = neow_rng.random_float() < 0.333_333_34;
    if force_rare {
        CardRarity::Rare
    } else {
        CardRarity::Uncommon
    }
}

fn neow_random_potion(potion_rng: &mut StsRng) -> Potion {
    let pick = potion_rng.random_int((IRONCLAD_POTION_POOL.len() - 1) as i32) as usize;
    IRONCLAD_POTION_POOL[pick]
}

fn neow_modeled_random_curse(card_rng: &mut StsRng) -> ContentId {
    random_normal_curse(card_rng)
}
