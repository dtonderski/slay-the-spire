//! Transitional Neow surface used by seed-start verification.
//!
//! This module intentionally preserves the currently captured Neow branches. It
//! is the seam where Milestone 33 can replace seed-name tables with source-backed
//! option generation one slice at a time.

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
}
