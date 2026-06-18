use crate::combat::CombatState;
use serde::{Deserialize, Serialize};

use crate::ids::ContentId;

/// Strength granted by [Relic::Vajra] at combat start.
pub const VAJRA_STRENGTH: i32 = 1;
/// Dexterity granted by [Relic::OddlySmoothStone] at combat start.
pub const ODDLY_SMOOTH_STONE_DEXTERITY: i32 = 1;

/// Content id for [Relic::Vajra].
pub const VAJRA_ID: ContentId = ContentId::new(300);
/// Content id for [Relic::OddlySmoothStone].
pub const ODDLY_SMOOTH_STONE_ID: ContentId = ContentId::new(301);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relic {
    Vajra,
    OddlySmoothStone,
}

impl Relic {
    #[must_use]
    pub fn content_id(self) -> ContentId {
        match self {
            Relic::Vajra => VAJRA_ID,
            Relic::OddlySmoothStone => ODDLY_SMOOTH_STONE_ID,
        }
    }

    #[must_use]
    pub fn from_content_id(id: ContentId) -> Option<Self> {
        match id {
            id if id == VAJRA_ID => Some(Relic::Vajra),
            id if id == ODDLY_SMOOTH_STONE_ID => Some(Relic::OddlySmoothStone),
            _ => None,
        }
    }
}

pub fn apply_start_of_combat_relics(combat: &mut CombatState, relics: &[Relic]) {
    for relic in relics {
        match relic {
            Relic::Vajra => {
                combat.player.powers.strength += VAJRA_STRENGTH;
            }
            Relic::OddlySmoothStone => {
                combat.player.powers.dexterity += ODDLY_SMOOTH_STONE_DEXTERITY;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatState;

    #[test]
    fn vajra_grants_one_strength_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::Vajra]);

        assert_eq!(combat.player.powers.strength, VAJRA_STRENGTH);
    }

    #[test]
    fn start_of_combat_relics_without_vajra_leaves_strength_unchanged() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[]);

        assert_eq!(combat.player.powers.strength, 0);
    }

    #[test]
    fn relic_round_trips_through_json() {
        let relic = Relic::Vajra;

        let json = serde_json::to_string(&relic).expect("relic serializes");
        let restored: Relic = serde_json::from_str(&json).expect("relic deserializes");

        assert_eq!(restored, relic);
    }

    #[test]
    fn oddly_smooth_stone_grants_one_dexterity_at_combat_start() {
        let mut combat = CombatState::initial_fixture();

        apply_start_of_combat_relics(&mut combat, &[Relic::OddlySmoothStone]);

        assert_eq!(combat.player.powers.dexterity, ODDLY_SMOOTH_STONE_DEXTERITY);
    }

    #[test]
    fn relic_content_ids_map_both_ways() {
        assert_eq!(Relic::Vajra.content_id(), VAJRA_ID);
        assert_eq!(Relic::OddlySmoothStone.content_id(), ODDLY_SMOOTH_STONE_ID);
        assert_eq!(Relic::from_content_id(VAJRA_ID), Some(Relic::Vajra));
        assert_eq!(
            Relic::from_content_id(ODDLY_SMOOTH_STONE_ID),
            Some(Relic::OddlySmoothStone)
        );
        assert_eq!(Relic::from_content_id(ContentId::new(999)), None);
    }
}
