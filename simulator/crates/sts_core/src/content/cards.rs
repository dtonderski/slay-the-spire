use crate::{
    card::{CardDefinition, CardType, CardValues, TargetRequirement},
    ContentId,
};

pub const STRIKE_R_ID: ContentId = ContentId::new(1);
pub const DEFEND_R_ID: ContentId = ContentId::new(2);
pub const BASH_ID: ContentId = ContentId::new(3);

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
};

pub const IRONCLAD_STARTER_CARDS: [CardDefinition; 3] = [STRIKE_R, DEFEND_R, BASH];

#[must_use]
pub fn get_card_definition(id: ContentId) -> Option<&'static CardDefinition> {
    IRONCLAD_STARTER_CARDS
        .iter()
        .find(|definition| definition.id == id)
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
