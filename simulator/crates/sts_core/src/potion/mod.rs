use crate::ids::ContentId;
use serde::{Deserialize, Serialize};

pub const MAX_POTIONS: usize = 3;

pub const FIRE_POTION_DAMAGE: i32 = 20;
pub const BLOCK_POTION_BLOCK: i32 = 12;

/// Content id for [Potion::Fire].
pub const FIRE_POTION_ID: ContentId = ContentId::new(200);
/// Content id for [Potion::Block].
pub const BLOCK_POTION_ID: ContentId = ContentId::new(201);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Potion {
    Fire,
    Block,
}

impl Potion {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Potion::Fire => FIRE_POTION_ID,
            Potion::Block => BLOCK_POTION_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == FIRE_POTION_ID => Some(Potion::Fire),
            id if id == BLOCK_POTION_ID => Some(Potion::Block),
            _ => None,
        }
    }

    #[must_use]
    pub fn requires_target(self) -> bool {
        matches!(self, Potion::Fire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_potion_round_trips_through_json() {
        let potion = Potion::Fire;

        let json = serde_json::to_string(&potion).expect("potion serializes");
        let restored: Potion = serde_json::from_str(&json).expect("potion deserializes");

        assert_eq!(restored, potion);
    }

    #[test]
    fn fire_potion_content_id_maps_both_ways() {
        assert_eq!(Potion::Fire.content_id(), FIRE_POTION_ID);
        assert_eq!(Potion::from_content_id(FIRE_POTION_ID), Some(Potion::Fire));
        assert_eq!(Potion::from_content_id(ContentId::new(999)), None);
    }

    #[test]
    fn block_potion_round_trips_through_json() {
        let potion = Potion::Block;

        let json = serde_json::to_string(&potion).expect("potion serializes");
        let restored: Potion = serde_json::from_str(&json).expect("potion deserializes");

        assert_eq!(restored, potion);
    }

    #[test]
    fn block_potion_content_id_maps_both_ways() {
        assert_eq!(Potion::Block.content_id(), BLOCK_POTION_ID);
        assert_eq!(
            Potion::from_content_id(BLOCK_POTION_ID),
            Some(Potion::Block)
        );
    }

    #[test]
    fn block_potion_does_not_require_target() {
        assert!(Potion::Fire.requires_target());
        assert!(!Potion::Block.requires_target());
    }
}
