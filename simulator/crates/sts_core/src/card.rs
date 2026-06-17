use crate::ids::{CardId, ContentId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: ContentId,
    pub key: &'static str,
    pub name: &'static str,
    pub cost: u8,
    pub card_type: CardType,
    pub target: TargetRequirement,
    pub values: CardValues,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardType {
    Attack,
    Skill,
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetRequirement {
    Enemy,
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
