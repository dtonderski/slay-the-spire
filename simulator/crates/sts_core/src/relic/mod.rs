use crate::combat::CombatState;
use serde::{Deserialize, Serialize};

/// Strength granted by [Relic::Vajra] at combat start.
pub const VAJRA_STRENGTH: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relic {
    Vajra,
}

pub fn apply_start_of_combat_relics(combat: &mut CombatState, relics: &[Relic]) {
    for relic in relics {
        match relic {
            Relic::Vajra => {
                combat.player.powers.strength += VAJRA_STRENGTH;
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
}
