use crate::ids::{CardId, ContentId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CardKeywords {
    pub ethereal: bool,
    pub exhaust: bool,
    pub retain: bool,
    pub unplayable: bool,
}

pub const CARD_KEYWORDS_NONE: CardKeywords = CardKeywords {
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
}

impl CardInstance {
    #[must_use]
    pub const fn new(id: CardId, content_id: ContentId) -> Self {
        Self { id, content_id }
    }
}
