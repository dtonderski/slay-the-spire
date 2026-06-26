//! Transitional Neow surface used by seed-start verification.
//!
//! This module intentionally preserves the currently captured Neow branches. It
//! is the seam where Milestone 33 can replace seed-name tables with source-backed
//! option generation one slice at a time.

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
    run::state::{RunRngStream, RunState},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnownNeowBranch {
    CommonRelic,
    ColorlessCardReward,
    TransformCard,
    NeowsLament,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownNeowScreen {
    pub options: Vec<&'static str>,
    pub branch: Option<KnownNeowBranch>,
    pub unchosen_command: &'static str,
    pub unchosen_reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownNeowColorlessReward {
    pub choice_names: Vec<&'static str>,
    pub card_ids: Vec<&'static str>,
    pub pick_index: usize,
    pub picked_card_id: &'static str,
    pub pick_label: &'static str,
}

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
        NeowRewardType::ThreeRareCards => neow_unique_ironclad_cards(rng, CardRarity::Rare, 3),
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
    run.relics.retain(|relic| *relic != Relic::BurningBlood);
    run.relic_keys.retain(|key| *key != RelicKey::BurningBlood);
    run.ensure_ironclad_relic_pools();

    let context = run.relic_spawn_context(run.current_floor, false);
    let relic = run
        .relic_pools
        .as_mut()
        .expect("relic pools initialized")
        .return_random_relic(RelicTier::Boss, &context);
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
        NeowRewardType::RemoveCard => "remove a card".to_owned(),
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
        NeowRewardType::TwoFiftyGold => "obtain 250 gold".to_owned(),
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

fn neow_unique_ironclad_cards(
    rng: &mut StsRng,
    rarity: CardRarity,
    count: usize,
) -> Vec<ContentId> {
    let mut cards = Vec::new();
    while cards.len() < count {
        let candidate = neow_random_ironclad_card(rng, rarity);
        if !cards.contains(&candidate) {
            cards.push(candidate);
        }
    }
    cards
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

pub fn known_neow_screen_for_seed(seed: &str) -> KnownNeowScreen {
    match seed {
        "M290001" => KnownNeowScreen {
            options: vec![
                "transform a card",
                "enemies in your next three combats have 1 hp",
                "obtain a curse max hp +16",
                "lose your starting relic obtain a random boss relic",
            ],
            branch: Some(KnownNeowBranch::TransformCard),
            unchosen_command: "CHOOSE 1/2/3",
            unchosen_reason: "unchosen Neow branches are classified with generated/source-backed partial support; selected-trace parity remains caveated for Neow's Lament, curse max-hp bonus, and boss swap",
        },
        "M290008" => KnownNeowScreen {
            options: vec![
                "transform a card",
                "obtain 100 gold",
                "lose all gold max hp +16",
                "lose your starting relic obtain a random boss relic",
            ],
            branch: Some(KnownNeowBranch::TransformCard),
            unchosen_command: "CHOOSE 1/2/3",
            unchosen_reason: "unchosen Neow branches are classified with generated/source-backed partial support; selected-trace parity remains caveated for gold, all-gold max-hp bonus, and boss swap",
        },
        "TEST" => KnownNeowScreen {
            options: vec![
                "choose a colorless card to obtain",
                "enemies in your next three combats have 1 hp",
                "lose 8 max hp obtain a random rare relic",
                "lose your starting relic obtain a random boss relic",
            ],
            branch: Some(KnownNeowBranch::ColorlessCardReward),
            unchosen_command: "CHOOSE 1/2/3",
            unchosen_reason: "unchosen Neow branches are classified with generated/source-backed partial support; selected-trace parity remains caveated for Neow's Lament, max-hp rare relic, and boss swap",
        },
        "CODEX04" => KnownNeowScreen {
            options: vec![
                "choose a colorless card to obtain",
                "obtain 3 random potions",
                "lose 8 max hp remove 2 cards",
                "lose your starting relic obtain a random boss relic",
            ],
            branch: Some(KnownNeowBranch::ColorlessCardReward),
            unchosen_command: "CHOOSE 1/2/3",
            unchosen_reason: "unchosen Neow branches are classified with generated/source-backed partial support; selected-trace parity remains caveated for potions, max-hp removal, and boss swap",
        },
        "CODEX03" => KnownNeowScreen {
            options: vec![
                "upgrade a card",
                "enemies in your next three combats have 1 hp",
                "lose all gold obtain a random rare relic",
                "lose your starting relic obtain a random boss relic",
            ],
            branch: Some(KnownNeowBranch::NeowsLament),
            unchosen_command: "CHOOSE 0/2/3",
            unchosen_reason: "unchosen Neow branches are classified with generated/source-backed partial support; selected-trace parity remains caveated for card upgrade, gold-for-relic, and boss swap",
        },
        _ => KnownNeowScreen {
            options: vec![
                "choose a card to obtain",
                "obtain a random common relic",
                "lose 8 max hp remove 2 cards",
                "lose your starting relic obtain a random boss relic",
            ],
            branch: Some(KnownNeowBranch::CommonRelic),
            unchosen_command: "CHOOSE 0/2/3",
            unchosen_reason: "unchosen Neow branches are classified with generated/source-backed partial support; selected-trace parity remains caveated for card reward, max-hp removal, and boss swap",
        },
    }
}

pub fn known_neow_colorless_reward_for_seed(seed: &str) -> Option<KnownNeowColorlessReward> {
    match seed {
        "TEST" => Some(KnownNeowColorlessReward {
            choice_names: vec!["deep breath", "swift strike", "jack of all trades"],
            card_ids: vec!["Deep Breath", "Swift Strike", "Jack Of All Trades"],
            pick_index: 1,
            picked_card_id: "Swift Strike",
            pick_label: "Neow Swift Strike pickup",
        }),
        "CODEX04" => Some(KnownNeowColorlessReward {
            choice_names: vec!["deep breath", "dramatic entrance", "jack of all trades"],
            card_ids: vec!["Deep Breath", "Dramatic Entrance", "Jack Of All Trades"],
            pick_index: 1,
            picked_card_id: "Dramatic Entrance",
            pick_label: "Neow Dramatic Entrance pickup",
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{
        is_curse_content_id, DEEP_BREATH_ID, DRAMATIC_ENTRANCE_ID, JACK_OF_ALL_TRADES_ID,
        SENTINEL_ID, SEVER_SOUL_ID, STRIKE_R_ID, SWIFT_STRIKE_ID,
    };
    use crate::content::reward_pool::NORMAL_CURSE_POOL;
    use crate::relic::{RelicPoolState, DARKSTONE_PERIAPT_MAX_HP};
    use crate::run::GridPurpose;

    #[test]
    fn known_verify01_common_relic_branch_matches_current_verifier_scope() {
        let screen = known_neow_screen_for_seed("VERIFY01");

        assert_eq!(screen.branch, Some(KnownNeowBranch::CommonRelic));
        assert_eq!(screen.options[1], "obtain a random common relic");
        assert!(screen.unchosen_reason.contains("card reward"));
    }

    #[test]
    fn known_codex04_colorless_branch_matches_captured_choices() {
        let screen = known_neow_screen_for_seed("CODEX04");
        let reward = known_neow_colorless_reward_for_seed("CODEX04").expect("reward");

        assert_eq!(screen.branch, Some(KnownNeowBranch::ColorlessCardReward));
        assert_eq!(screen.options[0], "choose a colorless card to obtain");
        assert_eq!(
            reward.card_ids,
            vec!["Deep Breath", "Dramatic Entrance", "Jack Of All Trades"]
        );
        assert_eq!(reward.pick_index, 1);
        assert_eq!(reward.picked_card_id, "Dramatic Entrance");
    }

    #[test]
    fn known_codex03_neows_lament_branch_matches_current_verifier_scope() {
        let screen = known_neow_screen_for_seed("CODEX03");

        assert_eq!(screen.branch, Some(KnownNeowBranch::NeowsLament));
        assert_eq!(
            screen.options[1],
            "enemies in your next three combats have 1 hp"
        );
    }

    #[test]
    fn known_transform_branches_preserve_captured_replacements() {
        assert_eq!(
            known_neow_screen_for_seed("M290001").branch,
            Some(KnownNeowBranch::TransformCard)
        );
        assert_eq!(
            known_neow_screen_for_seed("M290008").branch,
            Some(KnownNeowBranch::TransformCard)
        );
    }

    #[test]
    fn source_backed_generation_matches_verify01_captured_options() {
        let labels: Vec<_> = generate_neow_options(1_957_307_888_551, 80)
            .into_iter()
            .map(|option| option.label)
            .collect();

        assert_eq!(
            labels,
            vec![
                "choose a card to obtain",
                "obtain a random common relic",
                "lose 8 max hp remove 2 cards",
                "lose your starting relic obtain a random boss relic",
            ]
        );
    }

    #[test]
    fn source_backed_generation_matches_codex04_captured_options() {
        let labels: Vec<_> = generate_neow_options(22_079_335_079, 80)
            .into_iter()
            .map(|option| option.label)
            .collect();

        assert_eq!(
            labels,
            vec![
                "choose a colorless card to obtain",
                "obtain 3 random potions",
                "lose 8 max hp remove 2 cards",
                "lose your starting relic obtain a random boss relic",
            ]
        );
    }

    #[test]
    fn source_backed_generation_consumes_five_neow_rng_draws() {
        let mut rng = StsRng::new(22_079_335_079);

        for slot in 0..4 {
            generate_neow_option(slot, 80, &mut rng);
        }

        assert_eq!(rng.counter(), 5);
    }

    #[test]
    fn one_random_rare_card_consumes_one_neow_rng_draw_after_options() {
        let reward =
            generate_neow_card_reward(1_957_307_888_551, NeowRewardType::OneRandomRareCard);

        assert_eq!(reward.cards.len(), 1);
        assert_eq!(reward.neow_rng_counter, 6);
    }

    #[test]
    fn three_rare_cards_are_unique_and_consume_at_least_three_neow_rng_draws() {
        let reward = generate_neow_card_reward(1_957_307_888_551, NeowRewardType::ThreeRareCards);

        assert_eq!(reward.cards.len(), 3);
        assert_ne!(reward.cards[0], reward.cards[1]);
        assert_ne!(reward.cards[0], reward.cards[2]);
        assert_ne!(reward.cards[1], reward.cards[2]);
        assert!(reward.neow_rng_counter >= 8);
    }

    #[test]
    fn three_card_reward_rolls_common_or_uncommon_for_each_card() {
        let reward = generate_neow_card_reward(1_957_307_888_551, NeowRewardType::ThreeCards);

        assert_eq!(reward.cards.len(), 3);
        assert!(reward.neow_rng_counter >= 11);
    }

    #[test]
    fn colorless_reward_burns_neow_rarity_rolls_and_card_rng_identity_draws() {
        let reward =
            generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorless);

        assert_eq!(reward.cards.len(), 3);
        assert_eq!(reward.neow_rng_counter, 8);
        assert!(reward.card_rng_counter >= 3);
    }

    #[test]
    fn rare_colorless_reward_still_burns_neow_rarity_rolls() {
        let reward =
            generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorlessTwo);

        assert_eq!(reward.cards.len(), 3);
        assert_eq!(reward.neow_rng_counter, 8);
        assert!(reward.card_rng_counter >= 3);
    }

    #[test]
    fn colorless_reward_can_start_from_existing_card_rng_counter() {
        let unshifted =
            generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorlessTwo);
        let shifted = generate_neow_colorless_reward_with_card_rng_counter(
            22_079_335_079,
            NeowRewardType::RandomColorlessTwo,
            1,
        );

        assert_eq!(shifted.neow_rng_counter, unshifted.neow_rng_counter);
        assert_ne!(shifted.cards, unshifted.cards);
        assert!(shifted.card_rng_counter > unshifted.card_rng_counter);
    }

    #[test]
    fn generated_codex04_colorless_choices_match_captured_trace() {
        let reward =
            generate_neow_colorless_reward(22_079_335_079, NeowRewardType::RandomColorless);

        assert_eq!(
            reward.cards,
            vec![DEEP_BREATH_ID, DRAMATIC_ENTRANCE_ID, JACK_OF_ALL_TRADES_ID]
        );
    }

    #[test]
    fn generated_test_colorless_choices_match_captured_trace() {
        let reward = generate_neow_colorless_reward(1_218_623, NeowRewardType::RandomColorless);

        assert_eq!(
            reward.cards,
            vec![DEEP_BREATH_ID, SWIFT_STRIKE_ID, JACK_OF_ALL_TRADES_ID]
        );
    }

    #[test]
    fn three_potion_reward_rolls_direct_full_potion_pool_three_times() {
        let reward = generate_neow_three_potions(22_079_335_079);

        assert_eq!(reward.potions.len(), 3);
        assert_eq!(reward.potion_rng_counter, 3);
    }

    #[test]
    fn boss_swap_removes_starter_relic_before_popping_boss_pool() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::BurningBlood];
        run.relic_pools = Some(RelicPoolState {
            common: Vec::new(),
            uncommon: Vec::new(),
            rare: Vec::new(),
            shop: Vec::new(),
            boss: vec![RelicKey::BlackBlood, RelicKey::CoffeeDripper],
        });

        let reward = apply_neow_boss_swap(&mut run);

        assert_eq!(reward.relic, RelicKey::CoffeeDripper);
        assert!(!run.relics.contains(&Relic::BurningBlood));
        assert!(!run.relics.contains(&Relic::BlackBlood));
        assert!(run.relics.contains(&Relic::CoffeeDripper));
        assert_eq!(run.energy_per_turn, 4);
    }

    #[test]
    fn boss_swap_initializes_relic_pools_once_without_reward_rng_draws() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::BurningBlood];
        run.relic_rng_seed = 22_079_335_079;

        let reward = apply_neow_boss_swap(&mut run);

        assert_eq!(reward.relic_rng_counter, 5);
        assert_eq!(run.relic_rng_counter, 5);
        assert!(!run.relics.contains(&Relic::BurningBlood));
        assert!(run.relics.iter().any(|relic| relic.key() == reward.relic));
    }

    #[test]
    fn transform_reward_consumes_neow_rng_after_option_generation() {
        let reward = generate_neow_transform_reward(40_560_393_126, &[STRIKE_R_ID]);

        assert_eq!(reward.cards, vec![SEVER_SOUL_ID]);
        assert_eq!(reward.neow_rng_counter, 6);
    }

    #[test]
    fn transform_reward_matches_m290008_captured_replacement() {
        let reward = generate_neow_transform_reward(40_560_393_133, &[STRIKE_R_ID]);

        assert_eq!(reward.cards, vec![SENTINEL_ID]);
        assert_eq!(reward.neow_rng_counter, 6);
    }

    #[test]
    fn transform_reward_excludes_source_for_one_and_two_transforms() {
        let one = generate_neow_transform_reward(40_560_393_133, &[STRIKE_R_ID]);
        let two = generate_neow_transform_reward(40_560_393_133, &[STRIKE_R_ID, STRIKE_R_ID]);

        assert!(one.cards.iter().all(|card| *card != STRIKE_R_ID));
        assert!(two.cards.iter().all(|card| *card != STRIKE_R_ID));
        assert_eq!(one.neow_rng_counter, 6);
        assert_eq!(two.neow_rng_counter, 7);
    }

    #[test]
    fn transform_two_reward_consumes_one_neow_rng_draw_per_source() {
        let reward = generate_neow_transform_reward(40_560_393_126, &[STRIKE_R_ID, STRIKE_R_ID]);

        assert_eq!(reward.cards.len(), 2);
        assert_eq!(reward.neow_rng_counter, 7);
        assert!(reward.cards.iter().all(|card| *card != STRIKE_R_ID));
    }

    #[test]
    fn random_common_relic_initializes_pool_and_grants_verify01_toy_ornithopter() {
        let mut run = RunState::map_fixture();
        run.relic_rng_seed = 1_957_307_888_551;

        let reward = apply_neow_relic_reward(&mut run, NeowRewardType::RandomCommonRelic);

        assert_eq!(reward.relic, RelicKey::ToyOrnithopter);
        assert_eq!(reward.relic_rng_counter, 5);
        assert!(run.relics.contains(&Relic::ToyOrnithopter));
    }

    #[test]
    fn one_rare_relic_pops_from_rare_pool_without_tier_roll() {
        let mut run = RunState::map_fixture();
        run.relic_rng_counter = 17;
        run.relic_pools = Some(RelicPoolState {
            common: Vec::new(),
            uncommon: Vec::new(),
            rare: vec![RelicKey::Ginger, RelicKey::OldCoin],
            shop: Vec::new(),
            boss: Vec::new(),
        });

        let reward = apply_neow_relic_reward(&mut run, NeowRewardType::OneRareRelic);

        assert_eq!(reward.relic, RelicKey::Ginger);
        assert_eq!(reward.relic_rng_counter, 17);
        assert!(run.relics.contains(&Relic::Ginger));
        assert!(!run.relics.contains(&Relic::OldCoin));
    }

    #[test]
    fn neow_relic_reward_does_not_advance_counter_when_pools_are_initialized() {
        let mut run = RunState::map_fixture();
        run.relic_rng_counter = 23;
        run.relic_pools = Some(RelicPoolState {
            common: vec![RelicKey::ToyOrnithopter],
            uncommon: Vec::new(),
            rare: Vec::new(),
            shop: Vec::new(),
            boss: Vec::new(),
        });

        let reward = apply_neow_relic_reward(&mut run, NeowRewardType::RandomCommonRelic);

        assert_eq!(reward.relic, RelicKey::ToyOrnithopter);
        assert_eq!(reward.relic_rng_counter, 23);
        assert_eq!(run.relic_rng_counter, 23);
    }

    #[test]
    fn simple_gold_rewards_use_run_gold_gain() {
        let mut run = RunState::map_fixture();
        apply_neow_simple_reward(&mut run, NeowRewardType::HundredGold);
        assert_eq!(run.gold, 199);

        apply_neow_simple_reward(&mut run, NeowRewardType::TwoFiftyGold);
        assert_eq!(run.gold, 449);
    }

    #[test]
    fn simple_gold_rewards_respect_ectoplasm() {
        let mut run = RunState::map_fixture();
        run.relics = vec![Relic::Ectoplasm];

        apply_neow_simple_reward(&mut run, NeowRewardType::HundredGold);

        assert_eq!(run.gold, 99);
    }

    #[test]
    fn hp_bonus_rewards_increase_current_and_max_hp() {
        let mut run = RunState::map_fixture();
        run.player_hp = 50;

        apply_neow_simple_reward(&mut run, NeowRewardType::TenPercentHpBonus);
        assert_eq!(run.player_max_hp, 88);
        assert_eq!(run.player_hp, 58);

        apply_neow_simple_reward(&mut run, NeowRewardType::TwentyPercentHpBonus);
        assert_eq!(run.player_max_hp, 105);
        assert_eq!(run.player_hp, 75);
    }

    #[test]
    fn hp_loss_drawback_caps_current_hp_to_new_max() {
        let mut run = RunState::map_fixture();

        apply_neow_simple_drawback(&mut run, NeowDrawback::TenPercentHpLoss);

        assert_eq!(run.player_max_hp, 72);
        assert_eq!(run.player_hp, 72);
    }

    #[test]
    fn percent_damage_drawback_uses_thirty_percent_of_max_hp_and_keeps_one_hp() {
        let mut run = RunState::map_fixture();
        run.player_hp = 80;

        apply_neow_simple_drawback(&mut run, NeowDrawback::PercentDamage);
        assert_eq!(run.player_hp, 56);

        apply_neow_simple_drawback(&mut run, NeowDrawback::PercentDamage);
        apply_neow_simple_drawback(&mut run, NeowDrawback::PercentDamage);
        apply_neow_simple_drawback(&mut run, NeowDrawback::PercentDamage);
        assert_eq!(run.player_hp, 1);
    }

    #[test]
    fn no_gold_drawback_sets_gold_to_zero() {
        let mut run = RunState::map_fixture();

        apply_neow_simple_drawback(&mut run, NeowDrawback::NoGold);

        assert_eq!(run.gold, 0);
    }

    #[test]
    fn curse_drawback_uses_card_rng_not_neow_or_card_random_rng() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 40_560_393_126;
        run.card_rng_counter = 5;
        run.card_random_rng_counter = 11;
        let starting_len = run.deck.len();

        let drawback = apply_neow_curse_drawback(&mut run);

        assert!(is_curse_content_id(drawback.curse));
        assert!(NORMAL_CURSE_POOL.contains(&drawback.curse));
        assert_eq!(drawback.card_rng_counter, 6);
        assert_eq!(run.card_rng_counter, 6);
        assert_eq!(run.card_random_rng_counter, 11);
        assert_eq!(run.deck.len(), starting_len + 1);
        assert_eq!(run.deck.last().expect("curse").content_id, drawback.curse);
    }

    #[test]
    fn curse_drawback_consumes_card_rng_even_when_omamori_prevents_card() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        run.relics.push(Relic::Omamori);
        let starting_len = run.deck.len();

        let drawback = apply_neow_curse_drawback(&mut run);

        assert!(is_curse_content_id(drawback.curse));
        assert_eq!(drawback.card_rng_counter, 1);
        assert_eq!(run.card_rng_counter, 1);
        assert_eq!(run.omamori_charges_used, 1);
        assert_eq!(run.deck.len(), starting_len);
    }

    #[test]
    fn curse_drawback_runs_card_added_relic_hooks() {
        let mut run = RunState::map_fixture();
        run.reward_rng_seed = 1_218_623;
        run.relics.push(Relic::DarkstonePeriapt);
        let starting_max_hp = run.player_max_hp;
        let starting_hp = run.player_hp;

        let drawback = apply_neow_curse_drawback(&mut run);

        assert!(is_curse_content_id(drawback.curse));
        assert_eq!(
            run.player_max_hp,
            starting_max_hp + DARKSTONE_PERIAPT_MAX_HP
        );
        assert_eq!(run.player_hp, starting_hp + DARKSTONE_PERIAPT_MAX_HP);
    }

    #[test]
    fn grid_rewards_open_neow_specific_grids() {
        let mut remove = RunState::map_fixture();
        open_neow_reward_grid(&mut remove, NeowRewardType::RemoveCard);
        assert_eq!(
            remove.card_grid.as_ref().expect("remove grid").purpose,
            GridPurpose::NeowRemove { remaining: 1 }
        );

        let mut remove_two = RunState::map_fixture();
        open_neow_reward_grid(&mut remove_two, NeowRewardType::RemoveTwo);
        assert_eq!(
            remove_two
                .card_grid
                .as_ref()
                .expect("remove two grid")
                .purpose,
            GridPurpose::NeowRemove { remaining: 2 }
        );

        let mut upgrade = RunState::map_fixture();
        open_neow_reward_grid(&mut upgrade, NeowRewardType::UpgradeCard);
        assert_eq!(
            upgrade.card_grid.as_ref().expect("upgrade grid").purpose,
            GridPurpose::NeowUpgrade
        );
    }
}
