//! Transitional Neow surface used by seed-start verification.
//!
//! This module intentionally preserves the currently captured Neow branches. It
//! is the seam where Milestone 33 can replace seed-name tables with source-backed
//! option generation one slice at a time.

use crate::{
    card::CardRarity,
    content::{reward_pool::IRONCLAD_REWARD_ENTRIES, shop_pool::random_colorless_from_pool},
    ids::ContentId,
    potion::{Potion, IRONCLAD_POTION_POOL},
    rng::StsRng,
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
            unchosen_reason: "unchosen Neow branches are classified but not implemented: Neow's Lament, curse max-hp bonus, and boss swap",
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
            unchosen_reason: "unchosen Neow branches are classified but not implemented: gold, all-gold max-hp bonus, and boss swap",
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
            unchosen_reason: "unchosen Neow branches are classified but not implemented: Neow's Lament, max-hp rare relic, and boss swap",
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
            unchosen_reason: "unchosen Neow branches are classified but not implemented: potions, max-hp removal, and boss swap",
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
            unchosen_reason: "unchosen Neow branches are classified but not implemented: card upgrade, gold-for-relic, and boss swap",
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
            unchosen_reason: "unchosen Neow branches are classified but not implemented: card reward, max-hp removal, and boss swap",
        },
    }
}

pub fn known_neow_transformed_card(seed: &str) -> Option<&'static str> {
    match seed {
        "M290001" => Some("Sever Soul"),
        "M290008" => Some("Sentinel"),
        _ => None,
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
        DEEP_BREATH_ID, DRAMATIC_ENTRANCE_ID, JACK_OF_ALL_TRADES_ID, SWIFT_STRIKE_ID,
    };

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
        assert_eq!(known_neow_transformed_card("M290001"), Some("Sever Soul"));
        assert_eq!(known_neow_transformed_card("M290008"), Some("Sentinel"));
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
}
