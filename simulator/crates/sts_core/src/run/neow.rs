//! Transitional Neow surface used by seed-start verification.
//!
//! This module intentionally preserves the currently captured Neow branches. It
//! is the seam where Milestone 33 can replace seed-name tables with source-backed
//! option generation one slice at a time.

use crate::rng::StsRng;

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

pub fn generate_neow_options(numeric_seed: i64, player_max_hp: i32) -> Vec<GeneratedNeowOption> {
    let mut rng = StsRng::new(numeric_seed);
    (0..4)
        .map(|slot| generate_neow_option(slot, player_max_hp, &mut rng))
        .collect()
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
}
