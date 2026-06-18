use crate::{
    card::{
        CardDefinition, CardKeywords, CardType, CardValues, TargetRequirement, CARD_KEYWORDS_NONE,
    },
    ContentId,
};

pub const STRIKE_R_ID: ContentId = ContentId::new(1);
pub const DEFEND_R_ID: ContentId = ContentId::new(2);
pub const BASH_ID: ContentId = ContentId::new(3);
pub const WOUND_ID: ContentId = ContentId::new(4);
pub const DAZED_ID: ContentId = ContentId::new(5);
pub const BURN_ID: ContentId = ContentId::new(6);
pub const SLIMED_ID: ContentId = ContentId::new(7);
pub const ETHEREAL_STRIKE_ID: ContentId = ContentId::new(8);
pub const RETAIN_DEFEND_ID: ContentId = ContentId::new(9);
pub const ANGER_ID: ContentId = ContentId::new(10);
pub const CLEAVE_ID: ContentId = ContentId::new(11);
pub const TWIN_STRIKE_ID: ContentId = ContentId::new(12);
pub const ANGER_PLUS_ID: ContentId = ContentId::new(13);
pub const CLEAVE_PLUS_ID: ContentId = ContentId::new(14);
pub const TWIN_STRIKE_PLUS_ID: ContentId = ContentId::new(15);
pub const SHRUG_IT_OFF_ID: ContentId = ContentId::new(16);
pub const TRUE_GRIT_ID: ContentId = ContentId::new(17);
pub const BURNING_PACT_ID: ContentId = ContentId::new(18);
pub const FEEL_NO_PAIN_ID: ContentId = ContentId::new(19);
pub const DARK_EMBRACE_ID: ContentId = ContentId::new(20);
pub const POMMEL_STRIKE_ID: ContentId = ContentId::new(21);
pub const BATTLE_TRANCE_ID: ContentId = ContentId::new(22);
pub const SEEING_RED_ID: ContentId = ContentId::new(23);
pub const POMMEL_STRIKE_PLUS_ID: ContentId = ContentId::new(24);
pub const BATTLE_TRANCE_PLUS_ID: ContentId = ContentId::new(25);
pub const SEEING_RED_PLUS_ID: ContentId = ContentId::new(26);
pub const INFLAME_ID: ContentId = ContentId::new(27);
pub const FLEX_ID: ContentId = ContentId::new(28);
pub const SPOT_WEAKNESS_ID: ContentId = ContentId::new(29);
pub const INFLAME_PLUS_ID: ContentId = ContentId::new(30);
pub const FLEX_PLUS_ID: ContentId = ContentId::new(31);
pub const SPOT_WEAKNESS_PLUS_ID: ContentId = ContentId::new(32);
pub const WHIRLWIND_ID: ContentId = ContentId::new(33);
pub const WHIRLWIND_PLUS_ID: ContentId = ContentId::new(34);
pub const STRIKE_R_PLUS_ID: ContentId = ContentId::new(35);

pub const STRIKE_R: CardDefinition = CardDefinition {
    id: STRIKE_R_ID,
    key: "Strike_R",
    name: "Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const STRIKE_R_PLUS: CardDefinition = CardDefinition {
    id: STRIKE_R_PLUS_ID,
    key: "Strike_R+",
    name: "Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DEFEND_R: CardDefinition = CardDefinition {
    id: DEFEND_R_ID,
    key: "Defend_R",
    name: "Defend",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BASH: CardDefinition = CardDefinition {
    id: BASH_ID,
    key: "Bash",
    name: "Bash",
    cost: 2,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: Some(2),
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WOUND: CardDefinition = CardDefinition {
    id: WOUND_ID,
    key: "Wound",
    name: "Wound",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        unplayable: true,
        ethereal: false,
        exhaust: false,
        retain: false,
    },
};

pub const DAZED: CardDefinition = CardDefinition {
    id: DAZED_ID,
    key: "Dazed",
    name: "Dazed",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        unplayable: true,
        ethereal: false,
        exhaust: false,
        retain: false,
    },
};

/// Burn status deals this much HP loss per copy in hand at end of turn.
pub const BURN_END_TURN_DAMAGE: i32 = 2;

pub const BURN: CardDefinition = CardDefinition {
    id: BURN_ID,
    key: "Burn",
    name: "Burn",
    cost: 0,
    card_type: CardType::Status,
    target: TargetRequirement::None,
    values: CardValues {
        damage: Some(BURN_END_TURN_DAMAGE),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        unplayable: true,
        ethereal: false,
        exhaust: false,
        retain: false,
    },
};

pub const SLIMED: CardDefinition = CardDefinition {
    id: SLIMED_ID,
    key: "Slimed",
    name: "Slimed",
    cost: 1,
    card_type: CardType::Status,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(0),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        ethereal: false,
        exhaust: true,
        retain: false,
        unplayable: false,
    },
};

pub const ETHEREAL_STRIKE: CardDefinition = CardDefinition {
    id: ETHEREAL_STRIKE_ID,
    key: "Ethereal_Strike",
    name: "Ethereal Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CardKeywords {
        ethereal: true,
        exhaust: false,
        retain: false,
        unplayable: false,
    },
};

pub const RETAIN_DEFEND: CardDefinition = CardDefinition {
    id: RETAIN_DEFEND_ID,
    key: "Retain_Defend",
    name: "Retain Defend",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(5),
        vulnerable: None,
    },
    keywords: CardKeywords {
        ethereal: false,
        exhaust: false,
        retain: true,
        unplayable: false,
    },
};

pub const ANGER: CardDefinition = CardDefinition {
    id: ANGER_ID,
    key: "Anger",
    name: "Anger",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CLEAVE: CardDefinition = CardDefinition {
    id: CLEAVE_ID,
    key: "Cleave",
    name: "Cleave",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TWIN_STRIKE: CardDefinition = CardDefinition {
    id: TWIN_STRIKE_ID,
    key: "Twin Strike",
    name: "Twin Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const ANGER_PLUS: CardDefinition = CardDefinition {
    id: ANGER_PLUS_ID,
    key: "Anger+",
    name: "Anger+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(7),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const CLEAVE_PLUS: CardDefinition = CardDefinition {
    id: CLEAVE_PLUS_ID,
    key: "Cleave+",
    name: "Cleave+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TWIN_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: TWIN_STRIKE_PLUS_ID,
    key: "Twin Strike+",
    name: "Twin Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(6),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SHRUG_IT_OFF: CardDefinition = CardDefinition {
    id: SHRUG_IT_OFF_ID,
    key: "Shrug It Off",
    name: "Shrug It Off",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(8),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const TRUE_GRIT: CardDefinition = CardDefinition {
    id: TRUE_GRIT_ID,
    key: "True Grit",
    name: "True Grit",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: Some(7),
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BURNING_PACT: CardDefinition = CardDefinition {
    id: BURNING_PACT_ID,
    key: "Burning Pact",
    name: "Burning Pact",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FEEL_NO_PAIN: CardDefinition = CardDefinition {
    id: FEEL_NO_PAIN_ID,
    key: "Feel No Pain",
    name: "Feel No Pain",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const DARK_EMBRACE: CardDefinition = CardDefinition {
    id: DARK_EMBRACE_ID,
    key: "Dark Embrace",
    name: "Dark Embrace",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const POMMEL_STRIKE: CardDefinition = CardDefinition {
    id: POMMEL_STRIKE_ID,
    key: "Pommel Strike",
    name: "Pommel Strike",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(9),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BATTLE_TRANCE: CardDefinition = CardDefinition {
    id: BATTLE_TRANCE_ID,
    key: "Battle Trance",
    name: "Battle Trance",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SEEING_RED: CardDefinition = CardDefinition {
    id: SEEING_RED_ID,
    key: "Seeing Red",
    name: "Seeing Red",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const POMMEL_STRIKE_PLUS: CardDefinition = CardDefinition {
    id: POMMEL_STRIKE_PLUS_ID,
    key: "Pommel Strike+",
    name: "Pommel Strike+",
    cost: 1,
    card_type: CardType::Attack,
    target: TargetRequirement::Enemy,
    values: CardValues {
        damage: Some(12),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const BATTLE_TRANCE_PLUS: CardDefinition = CardDefinition {
    id: BATTLE_TRANCE_PLUS_ID,
    key: "Battle Trance+",
    name: "Battle Trance+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SEEING_RED_PLUS: CardDefinition = CardDefinition {
    id: SEEING_RED_PLUS_ID,
    key: "Seeing Red+",
    name: "Seeing Red+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const INFLAME: CardDefinition = CardDefinition {
    id: INFLAME_ID,
    key: "Inflame",
    name: "Inflame",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FLEX: CardDefinition = CardDefinition {
    id: FLEX_ID,
    key: "Flex",
    name: "Flex",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SPOT_WEAKNESS: CardDefinition = CardDefinition {
    id: SPOT_WEAKNESS_ID,
    key: "Spot Weakness",
    name: "Spot Weakness",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const INFLAME_PLUS: CardDefinition = CardDefinition {
    id: INFLAME_PLUS_ID,
    key: "Inflame+",
    name: "Inflame+",
    cost: 1,
    card_type: CardType::Power,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const FLEX_PLUS: CardDefinition = CardDefinition {
    id: FLEX_PLUS_ID,
    key: "Flex+",
    name: "Flex+",
    cost: 0,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const SPOT_WEAKNESS_PLUS: CardDefinition = CardDefinition {
    id: SPOT_WEAKNESS_PLUS_ID,
    key: "Spot Weakness+",
    name: "Spot Weakness+",
    cost: 1,
    card_type: CardType::Skill,
    target: TargetRequirement::None,
    values: CardValues {
        damage: None,
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WHIRLWIND: CardDefinition = CardDefinition {
    id: WHIRLWIND_ID,
    key: "Whirlwind",
    name: "Whirlwind",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(5),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const WHIRLWIND_PLUS: CardDefinition = CardDefinition {
    id: WHIRLWIND_PLUS_ID,
    key: "Whirlwind+",
    name: "Whirlwind+",
    cost: 0,
    card_type: CardType::Attack,
    target: TargetRequirement::AllEnemies,
    values: CardValues {
        damage: Some(8),
        block: None,
        vulnerable: None,
    },
    keywords: CARD_KEYWORDS_NONE,
};

pub const IRONCLAD_STARTER_CARDS: [CardDefinition; 3] = [STRIKE_R, DEFEND_R, BASH];
pub const STATUS_CARDS: [CardDefinition; 4] = [WOUND, DAZED, BURN, SLIMED];
pub const MECHANIC_TEST_CARDS: [CardDefinition; 2] = [ETHEREAL_STRIKE, RETAIN_DEFEND];
pub const MILESTONE5_ATTACK_CARDS: [CardDefinition; 10] = [
    ANGER,
    CLEAVE,
    TWIN_STRIKE,
    ANGER_PLUS,
    CLEAVE_PLUS,
    TWIN_STRIKE_PLUS,
    POMMEL_STRIKE,
    POMMEL_STRIKE_PLUS,
    WHIRLWIND,
    WHIRLWIND_PLUS,
];
pub const MILESTONE5_SKILL_CARDS: [CardDefinition; 11] = [
    SHRUG_IT_OFF,
    TRUE_GRIT,
    BURNING_PACT,
    BATTLE_TRANCE,
    SEEING_RED,
    BATTLE_TRANCE_PLUS,
    SEEING_RED_PLUS,
    FLEX,
    SPOT_WEAKNESS,
    FLEX_PLUS,
    SPOT_WEAKNESS_PLUS,
];
pub const MILESTONE5_POWER_CARDS: [CardDefinition; 4] =
    [FEEL_NO_PAIN, DARK_EMBRACE, INFLAME, INFLAME_PLUS];
pub const ALL_CARDS: [CardDefinition; 35] = [
    STRIKE_R,
    STRIKE_R_PLUS,
    DEFEND_R,
    BASH,
    WOUND,
    DAZED,
    BURN,
    SLIMED,
    ETHEREAL_STRIKE,
    RETAIN_DEFEND,
    ANGER,
    CLEAVE,
    TWIN_STRIKE,
    ANGER_PLUS,
    CLEAVE_PLUS,
    TWIN_STRIKE_PLUS,
    SHRUG_IT_OFF,
    TRUE_GRIT,
    BURNING_PACT,
    FEEL_NO_PAIN,
    DARK_EMBRACE,
    POMMEL_STRIKE,
    BATTLE_TRANCE,
    SEEING_RED,
    POMMEL_STRIKE_PLUS,
    BATTLE_TRANCE_PLUS,
    SEEING_RED_PLUS,
    INFLAME,
    FLEX,
    SPOT_WEAKNESS,
    INFLAME_PLUS,
    FLEX_PLUS,
    SPOT_WEAKNESS_PLUS,
    WHIRLWIND,
    WHIRLWIND_PLUS,
];

#[must_use]
pub fn get_card_definition(id: ContentId) -> Option<&'static CardDefinition> {
    ALL_CARDS.iter().find(|definition| definition.id == id)
}

/// Maps a base card content id to its upgraded (+) version, if one exists.
#[must_use]
pub fn upgrade_content_id(id: ContentId) -> Option<ContentId> {
    match id {
        STRIKE_R_ID => Some(STRIKE_R_PLUS_ID),
        ANGER_ID => Some(ANGER_PLUS_ID),
        CLEAVE_ID => Some(CLEAVE_PLUS_ID),
        TWIN_STRIKE_ID => Some(TWIN_STRIKE_PLUS_ID),
        POMMEL_STRIKE_ID => Some(POMMEL_STRIKE_PLUS_ID),
        BATTLE_TRANCE_ID => Some(BATTLE_TRANCE_PLUS_ID),
        SEEING_RED_ID => Some(SEEING_RED_PLUS_ID),
        INFLAME_ID => Some(INFLAME_PLUS_ID),
        FLEX_ID => Some(FLEX_PLUS_ID),
        SPOT_WEAKNESS_ID => Some(SPOT_WEAKNESS_PLUS_ID),
        WHIRLWIND_ID => Some(WHIRLWIND_PLUS_ID),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strike_r_has_expected_starter_values() {
        assert_eq!(STRIKE_R.cost, 1);
        assert_eq!(STRIKE_R.target, TargetRequirement::Enemy);
        assert_eq!(STRIKE_R.card_type, CardType::Attack);
        assert_eq!(STRIKE_R.values.damage, Some(6));
    }

    #[test]
    fn defend_r_has_expected_starter_values() {
        assert_eq!(DEFEND_R.cost, 1);
        assert_eq!(DEFEND_R.target, TargetRequirement::None);
        assert_eq!(DEFEND_R.card_type, CardType::Skill);
        assert_eq!(DEFEND_R.values.block, Some(5));
    }

    #[test]
    fn bash_has_expected_starter_values() {
        assert_eq!(BASH.cost, 2);
        assert_eq!(BASH.target, TargetRequirement::Enemy);
        assert_eq!(BASH.card_type, CardType::Attack);
        assert_eq!(BASH.values.damage, Some(8));
        assert_eq!(BASH.values.vulnerable, Some(2));
    }

    #[test]
    fn anger_has_expected_values() {
        assert_eq!(ANGER.cost, 0);
        assert_eq!(ANGER.target, TargetRequirement::Enemy);
        assert_eq!(ANGER.card_type, CardType::Attack);
        assert_eq!(ANGER.values.damage, Some(6));
    }

    #[test]
    fn cleave_has_expected_values() {
        assert_eq!(CLEAVE.cost, 1);
        assert_eq!(CLEAVE.target, TargetRequirement::AllEnemies);
        assert_eq!(CLEAVE.card_type, CardType::Attack);
        assert_eq!(CLEAVE.values.damage, Some(8));
    }

    #[test]
    fn twin_strike_has_expected_values() {
        assert_eq!(TWIN_STRIKE.cost, 1);
        assert_eq!(TWIN_STRIKE.target, TargetRequirement::Enemy);
        assert_eq!(TWIN_STRIKE.card_type, CardType::Attack);
        assert_eq!(TWIN_STRIKE.values.damage, Some(5));
    }

    #[test]
    fn strike_r_plus_deals_three_more_damage() {
        assert_eq!(STRIKE_R_PLUS.values.damage, Some(9));
    }

    #[test]
    fn anger_plus_deals_one_more_damage() {
        assert_eq!(ANGER_PLUS.values.damage, Some(7));
    }

    #[test]
    fn cleave_plus_deals_one_more_damage() {
        assert_eq!(CLEAVE_PLUS.values.damage, Some(9));
    }

    #[test]
    fn twin_strike_plus_deals_one_more_damage() {
        assert_eq!(TWIN_STRIKE_PLUS.values.damage, Some(6));
    }

    #[test]
    fn shrug_it_off_has_expected_values() {
        assert_eq!(SHRUG_IT_OFF.cost, 1);
        assert_eq!(SHRUG_IT_OFF.target, TargetRequirement::None);
        assert_eq!(SHRUG_IT_OFF.card_type, CardType::Skill);
        assert_eq!(SHRUG_IT_OFF.values.block, Some(8));
    }

    #[test]
    fn true_grit_has_expected_values() {
        assert_eq!(TRUE_GRIT.cost, 1);
        assert_eq!(TRUE_GRIT.target, TargetRequirement::None);
        assert_eq!(TRUE_GRIT.card_type, CardType::Skill);
        assert_eq!(TRUE_GRIT.values.block, Some(7));
    }

    #[test]
    fn burning_pact_has_expected_values() {
        assert_eq!(BURNING_PACT.cost, 1);
        assert_eq!(BURNING_PACT.target, TargetRequirement::None);
        assert_eq!(BURNING_PACT.card_type, CardType::Skill);
    }

    #[test]
    fn feel_no_pain_has_expected_values() {
        assert_eq!(FEEL_NO_PAIN.cost, 1);
        assert_eq!(FEEL_NO_PAIN.target, TargetRequirement::None);
        assert_eq!(FEEL_NO_PAIN.card_type, CardType::Power);
    }

    #[test]
    fn dark_embrace_has_expected_values() {
        assert_eq!(DARK_EMBRACE.cost, 1);
        assert_eq!(DARK_EMBRACE.target, TargetRequirement::None);
        assert_eq!(DARK_EMBRACE.card_type, CardType::Power);
    }

    #[test]
    fn pommel_strike_has_expected_values() {
        assert_eq!(POMMEL_STRIKE.cost, 1);
        assert_eq!(POMMEL_STRIKE.target, TargetRequirement::Enemy);
        assert_eq!(POMMEL_STRIKE.card_type, CardType::Attack);
        assert_eq!(POMMEL_STRIKE.values.damage, Some(9));
    }

    #[test]
    fn battle_trance_has_expected_values() {
        assert_eq!(BATTLE_TRANCE.cost, 0);
        assert_eq!(BATTLE_TRANCE.target, TargetRequirement::None);
        assert_eq!(BATTLE_TRANCE.card_type, CardType::Skill);
    }

    #[test]
    fn seeing_red_has_expected_values() {
        assert_eq!(SEEING_RED.cost, 1);
        assert_eq!(SEEING_RED.target, TargetRequirement::None);
        assert_eq!(SEEING_RED.card_type, CardType::Skill);
    }

    #[test]
    fn pommel_strike_plus_deals_three_more_damage() {
        assert_eq!(POMMEL_STRIKE_PLUS.values.damage, Some(12));
    }

    #[test]
    fn battle_trance_plus_is_zero_cost_skill() {
        assert_eq!(BATTLE_TRANCE_PLUS.cost, 0);
        assert_eq!(BATTLE_TRANCE_PLUS.card_type, CardType::Skill);
    }

    #[test]
    fn seeing_red_plus_costs_zero() {
        assert_eq!(SEEING_RED_PLUS.cost, 0);
    }

    #[test]
    fn inflame_has_expected_values() {
        assert_eq!(INFLAME.cost, 1);
        assert_eq!(INFLAME.target, TargetRequirement::None);
        assert_eq!(INFLAME.card_type, CardType::Power);
    }

    #[test]
    fn flex_has_expected_values() {
        assert_eq!(FLEX.cost, 0);
        assert_eq!(FLEX.target, TargetRequirement::None);
        assert_eq!(FLEX.card_type, CardType::Skill);
    }

    #[test]
    fn spot_weakness_has_expected_values() {
        assert_eq!(SPOT_WEAKNESS.cost, 1);
        assert_eq!(SPOT_WEAKNESS.target, TargetRequirement::None);
        assert_eq!(SPOT_WEAKNESS.card_type, CardType::Skill);
    }

    #[test]
    fn inflame_plus_grants_one_more_strength_than_base() {
        assert_eq!(INFLAME_PLUS.card_type, CardType::Power);
    }

    #[test]
    fn flex_plus_is_zero_cost_skill() {
        assert_eq!(FLEX_PLUS.cost, 0);
        assert_eq!(FLEX_PLUS.card_type, CardType::Skill);
    }

    #[test]
    fn spot_weakness_plus_is_one_cost_skill() {
        assert_eq!(SPOT_WEAKNESS_PLUS.cost, 1);
        assert_eq!(SPOT_WEAKNESS_PLUS.card_type, CardType::Skill);
    }

    #[test]
    fn whirlwind_has_expected_values() {
        assert_eq!(WHIRLWIND.cost, 0);
        assert_eq!(WHIRLWIND.target, TargetRequirement::AllEnemies);
        assert_eq!(WHIRLWIND.card_type, CardType::Attack);
        assert_eq!(WHIRLWIND.values.damage, Some(5));
    }

    #[test]
    fn whirlwind_plus_deals_three_more_damage_per_hit() {
        assert_eq!(WHIRLWIND_PLUS.values.damage, Some(8));
    }
}
