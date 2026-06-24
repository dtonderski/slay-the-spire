use crate::ids::{CardId, ContentId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CardKeywords {
    pub innate: bool,
    pub ethereal: bool,
    pub exhaust: bool,
    pub retain: bool,
    pub unplayable: bool,
}

pub const CARD_KEYWORDS_NONE: CardKeywords = CardKeywords {
    innate: false,
    ethereal: false,
    exhaust: false,
    retain: false,
    unplayable: false,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardRarity {
    Common,
    Uncommon,
    Rare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: ContentId,
    pub key: &'static str,
    pub name: &'static str,
    pub cost: u8,
    pub card_type: CardType,
    pub target: TargetRequirement,
    pub values: CardValues,
    pub keywords: CardKeywords,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardType {
    Attack,
    Skill,
    Power,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetRequirement {
    Enemy,
    AllEnemies,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CardValues {
    pub damage: Option<i32>,
    pub block: Option<i32>,
    pub vulnerable: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardInstance {
    pub id: CardId,
    pub content_id: ContentId,
    /// Cards selected by bottled relics begin combat in the opening hand.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bottled: bool,
    /// Combat-only generated cards (for example Power Potion) may override printed cost.
    #[serde(default)]
    pub temp_cost: Option<u8>,
    /// Cards created only for the current combat vanish after play.
    #[serde(default)]
    pub combat_only: bool,
}

impl CardInstance {
    #[must_use]
    pub const fn new(id: CardId, content_id: ContentId) -> Self {
        Self {
            id,
            content_id,
            bottled: false,
            temp_cost: None,
            combat_only: false,
        }
    }

    #[must_use]
    pub const fn combat_generated(id: CardId, content_id: ContentId, temp_cost: u8) -> Self {
        Self {
            id,
            content_id,
            bottled: false,
            temp_cost: Some(temp_cost),
            combat_only: true,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}
