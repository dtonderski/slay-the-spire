use crate::ids::ContentId;
use serde::{Deserialize, Serialize};

pub const MAX_POTIONS: usize = 3;

/// Content id for [Potion::Fire] (placeholder reward; not usable in combat yet).
pub const FIRE_POTION_ID: ContentId = ContentId::new(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Potion {
    Fire,
}

impl Potion {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Potion::Fire => FIRE_POTION_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        if id == FIRE_POTION_ID {
            Some(Potion::Fire)
        } else {
            None
        }
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
}
