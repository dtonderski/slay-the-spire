use crate::{
    card::CardInstance,
    ids::{CardId, ContentId, MonsterId},
    SimError, SimResult, Snapshot, SNAPSHOT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatState {
    pub player: PlayerState,
    pub monsters: Vec<MonsterState>,
    pub piles: CardPiles,
    pub phase: CombatPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub hp: i32,
    pub block: i32,
    pub energy: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterState {
    pub id: MonsterId,
    pub hp: i32,
    pub block: i32,
    pub alive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardPiles {
    pub hand: Vec<CardInstance>,
    pub draw_pile: Vec<CardInstance>,
    pub discard_pile: Vec<CardInstance>,
    pub exhaust_pile: Vec<CardInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatPhase {
    WaitingForPlayer,
    MonsterTurn,
    Won,
    Lost,
}

impl CombatState {
    #[must_use]
    pub fn initial_fixture() -> Self {
        let strike = ContentId::new(1);
        let defend = ContentId::new(2);
        let bash = ContentId::new(3);

        Self {
            player: PlayerState {
                hp: 80,
                block: 0,
                energy: 3,
            },
            monsters: vec![MonsterState {
                id: MonsterId::new(1),
                hp: 40,
                block: 0,
                alive: true,
            }],
            piles: CardPiles {
                hand: vec![
                    CardInstance::new(CardId::new(1), strike),
                    CardInstance::new(CardId::new(2), defend),
                    CardInstance::new(CardId::new(3), bash),
                ],
                draw_pile: vec![CardInstance::new(CardId::new(4), strike)],
                discard_pile: Vec::new(),
                exhaust_pile: Vec::new(),
            },
            phase: CombatPhase::WaitingForPlayer,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot<Self> {
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: self.clone(),
        }
    }

    pub fn validate_unique_card_piles(&self) -> SimResult<()> {
        let mut seen = BTreeSet::new();
        for card in self.piles.all_cards() {
            if !seen.insert(card.id) {
                return Err(SimError::InvalidState(
                    "card instance appears in more than one pile",
                ));
            }
        }
        Ok(())
    }
}

impl CardPiles {
    fn all_cards(&self) -> impl Iterator<Item = &CardInstance> {
        self.hand
            .iter()
            .chain(self.draw_pile.iter())
            .chain(self.discard_pile.iter())
            .chain(self.exhaust_pile.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_fixture_serializes() {
        let state = CombatState::initial_fixture();

        let json = serde_json::to_string(&state).expect("combat state serializes");
        let restored: CombatState = serde_json::from_str(&json).expect("combat state deserializes");

        assert_eq!(restored, state);
    }

    #[test]
    fn snapshot_round_trip_preserves_state_hash() {
        let snapshot = CombatState::initial_fixture().snapshot();
        let hash_before = snapshot.hash().expect("snapshot hashes");
        let json = snapshot.canonical_json().expect("snapshot serializes");
        let restored: Snapshot<CombatState> =
            serde_json::from_str(&json).expect("snapshot deserializes");

        assert_eq!(restored.hash().expect("restored hashes"), hash_before);
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn fixture_card_instances_do_not_appear_in_two_piles() {
        let state = CombatState::initial_fixture();

        assert_eq!(state.validate_unique_card_piles(), Ok(()));
    }
}
