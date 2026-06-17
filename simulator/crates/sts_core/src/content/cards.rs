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

pub const IRONCLAD_STARTER_CARDS: [CardDefinition; 3] = [STRIKE_R, DEFEND_R, BASH];
pub const STATUS_CARDS: [CardDefinition; 4] = [WOUND, DAZED, BURN, SLIMED];
pub const MECHANIC_TEST_CARDS: [CardDefinition; 2] = [ETHEREAL_STRIKE, RETAIN_DEFEND];
pub const ALL_CARDS: [CardDefinition; 9] = [
    STRIKE_R,
    DEFEND_R,
    BASH,
    WOUND,
    DAZED,
    BURN,
    SLIMED,
    ETHEREAL_STRIKE,
    RETAIN_DEFEND,
];

#[must_use]
pub fn get_card_definition(id: ContentId) -> Option<&'static CardDefinition> {
    ALL_CARDS.iter().find(|definition| definition.id == id)
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
}
