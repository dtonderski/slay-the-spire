use crate::{
    card::CardInstance,
    content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    content::character::IRONCLAD_A0_BASE_HP,
    content::monsters::{
        monster_state, ACID_SLIME_A0, CULTIST_A0, FIXED_SIMPLE_MONSTER, GREEN_LOUSE_A0,
        GREMLIN_NOB_A0, HEXAGHOST_A0, JAW_WORM_A0, LAGAVULIN_A0, RED_LOUSE_A0, SENTRY_A0,
        SPIKE_SLIME_A0,
    },
    ids::{CardId, MonsterId},
    power::{MonsterPowers, PlayerPowers},
    ContentId, SimError, SimResult, Snapshot, SNAPSHOT_SCHEMA_VERSION,
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
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub powers: PlayerPowers,
    #[serde(default)]
    pub cannot_draw: bool,
    #[serde(default)]
    pub temp_strength: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterState {
    pub id: MonsterId,
    pub hp: i32,
    pub block: i32,
    pub alive: bool,
    pub powers: MonsterPowers,
    pub content_id: ContentId,
    #[serde(default)]
    pub moves_executed: u32,
    #[serde(default)]
    pub sleep_turns_remaining: u32,
    #[serde(default)]
    pub has_siphoned: bool,
    pub intent: MonsterIntent,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterIntent {
    Attack { damage: i32 },
    Block { block: i32 },
    Ritual { amount: i32 },
    AttackAndBlock { damage: i32, block: i32 },
    StrengthAndBlock { strength: i32, block: i32 },
    ApplyPlayerWeak { amount: i32 },
    Sleep,
    SiphonPlayer { strength: i32, dexterity: i32 },
    AddDazedToDiscard { count: i32 },
    AddBurnToDiscard { count: i32, damage: i32 },
    AttackMultiple { damage: i32, hits: i32 },
}

impl CombatState {
    #[must_use]
    pub fn initial_fixture() -> Self {
        Self {
            player: PlayerState {
                hp: IRONCLAD_A0_BASE_HP,
                max_hp: IRONCLAD_A0_BASE_HP,
                block: 0,
                energy: 3,
                powers: PlayerPowers::default(),
                cannot_draw: false,
                temp_strength: 0,
            },
            monsters: vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))],
            piles: CardPiles {
                hand: vec![
                    CardInstance::new(CardId::new(1), STRIKE_R_ID),
                    CardInstance::new(CardId::new(2), DEFEND_R_ID),
                    CardInstance::new(CardId::new(3), BASH_ID),
                ],
                draw_pile: vec![CardInstance::new(CardId::new(4), STRIKE_R_ID)],
                discard_pile: Vec::new(),
                exhaust_pile: Vec::new(),
            },
            phase: CombatPhase::WaitingForPlayer,
        }
    }

    #[must_use]
    pub fn cultist_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&CULTIST_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn jaw_worm_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&JAW_WORM_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn gremlin_nob_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&GREMLIN_NOB_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn red_louse_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&RED_LOUSE_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn green_louse_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&GREEN_LOUSE_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn spike_slime_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&SPIKE_SLIME_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn acid_slime_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&ACID_SLIME_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn lagavulin_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&LAGAVULIN_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn sentry_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![
            monster_state(&SENTRY_A0, MonsterId::new(1)),
            monster_state(&SENTRY_A0, MonsterId::new(2)),
            monster_state(&SENTRY_A0, MonsterId::new(3)),
        ];
        state
    }

    #[must_use]
    pub fn hexaghost_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&HEXAGHOST_A0, MonsterId::new(1))];
        state
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
    pub fn max_card_instance_id(&self) -> u64 {
        self.all_cards()
            .map(|card| card.id.get())
            .max()
            .unwrap_or(0)
    }

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

    #[test]
    fn player_temp_strength_round_trips_through_json() {
        let mut state = CombatState::initial_fixture();
        state.player.temp_strength = 2;

        let json = serde_json::to_string(&state.player).expect("player serializes");
        let restored: PlayerState = serde_json::from_str(&json).expect("player deserializes");

        assert_eq!(restored.temp_strength, 2);
    }
}
