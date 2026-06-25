use crate::{
    card::CardInstance,
    content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    content::character::IRONCLAD_A0_BASE_HP,
    content::monsters::{
        monster_state, ACID_SLIME_A0, CULTIST_A0, FIXED_SIMPLE_MONSTER, GREEN_LOUSE_A0,
        GREMLIN_NOB_A0, GUARDIAN_A0, HEXAGHOST_A0, JAW_WORM_A0, LAGAVULIN_A0, RED_LOUSE_A0,
        SENTRY_A0, SLIME_BOSS_A0, SPIKE_SLIME_A0,
    },
    ids::{CardId, MonsterId},
    power::{MonsterPowers, PlayerPowers},
    relic::{Relic, RelicCounters},
    rng::StsRng,
    ContentId, SimError, SimResult, Snapshot, SNAPSHOT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const BASE_PLAYER_ENERGY: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatState {
    pub player: PlayerState,
    pub monsters: Vec<MonsterState>,
    pub piles: CardPiles,
    pub phase: CombatPhase,
    #[serde(default)]
    pub relics: Vec<Relic>,
    #[serde(default)]
    pub relic_counters: RelicCounters,
    #[serde(default)]
    pub ascension: u8,
    #[serde(default)]
    pub shuffle_rng: Option<StsRng>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_random_rng: Option<StsRng>,
    /// In-combat zero-cost card reward from potions such as Power Potion.
    #[serde(default)]
    pub potion_card_reward: Option<Vec<CardInstance>>,
    /// In-combat normal-cost colorless reward from Toolbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolbox_card_reward: Option<Vec<CardInstance>>,
    /// Awaiting player choice for Warcry, Armaments, and similar hand-select effects.
    #[serde(default)]
    pub hand_select: Option<HandSelectState>,
    /// Awaiting player choice for discard-pile effects such as Liquid Memories or Headbutt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard_select: Option<DiscardSelectState>,
    /// Awaiting player choice for exhaust-related effects such as Elixir, Gambling Chip, or Exhume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exhaust_select: Option<ExhaustSelectState>,
    /// One-shot flag from Duplication Potion: the next played card resolves twice.
    #[serde(default, skip_serializing_if = "is_false")]
    pub duplication_potion_pending: bool,
    /// Pending Double Tap stacks: the next played Attack resolves twice per stack.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub double_tap_pending: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandSelectState {
    #[serde(default)]
    pub purpose: HandSelectPurpose,
    pub source_card_id: CardId,
    pub selected_hand_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HandSelectPurpose {
    #[default]
    WarcryPutOnDraw,
    ArmamentsUpgrade,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscardSelectState {
    #[serde(default)]
    pub purpose: DiscardSelectPurpose,
    pub source_card_id: Option<CardId>,
    pub selected_discard_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DiscardSelectPurpose {
    #[default]
    LiquidMemoriesReturnToHand,
    HeadbuttPutOnDraw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExhaustSelectState {
    #[serde(default)]
    pub purpose: ExhaustSelectPurpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_card_id: Option<CardId>,
    pub selected_hand_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExhaustSelectPurpose {
    #[default]
    Exhaust,
    GamblingChip,
    ExhumeReturnToHand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    #[serde(default = "default_player_energy")]
    pub max_energy: i32,
    pub powers: PlayerPowers,
    #[serde(default)]
    pub cannot_draw: bool,
    #[serde(default)]
    pub temp_strength: i32,
    #[serde(default)]
    pub temp_dexterity: i32,
    #[serde(default)]
    pub temp_thorns: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub temp_rage_block: i32,
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
    #[serde(default)]
    pub split_triggered: bool,
    #[serde(default)]
    pub defensive_turns_remaining: u32,
    #[serde(default)]
    pub mode_shift: i32,
    #[serde(default)]
    pub in_defensive_mode: bool,
    #[serde(default)]
    pub rolled_attack_damage: Option<i32>,
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
    AttackApplyPlayerVulnerable { damage: i32, vulnerable: i32 },
    Sleep,
    Stun,
    SiphonPlayer { strength: i32, dexterity: i32 },
    AddDazedToDiscard { count: i32 },
    AddBurnToDiscard { count: i32, damage: i32 },
    AttackMultiple { damage: i32, hits: i32 },
    DefensiveCharge { block: i32, strength: i32 },
}

impl CombatState {
    #[must_use]
    pub fn initial_fixture() -> Self {
        Self {
            player: PlayerState {
                hp: IRONCLAD_A0_BASE_HP,
                max_hp: IRONCLAD_A0_BASE_HP,
                block: 0,
                energy: BASE_PLAYER_ENERGY,
                max_energy: BASE_PLAYER_ENERGY,
                powers: PlayerPowers::default(),
                cannot_draw: false,
                temp_strength: 0,
                temp_dexterity: 0,
                temp_thorns: 0,
                temp_rage_block: 0,
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
            relics: Vec::new(),
            relic_counters: RelicCounters::default(),
            ascension: 0,
            shuffle_rng: None,
            card_random_rng: None,
            potion_card_reward: None,
            toolbox_card_reward: None,
            hand_select: None,
            discard_select: None,
            exhaust_select: None,
            duplication_potion_pending: false,
            double_tap_pending: 0,
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
    pub fn slime_boss_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&SLIME_BOSS_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn guardian_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&GUARDIAN_A0, MonsterId::new(1))];
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

fn is_false(value: &bool) -> bool {
    !*value
}

fn default_player_energy() -> i32 {
    BASE_PLAYER_ENERGY
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
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
        state.player.temp_thorns = 4;
        state.player.temp_rage_block = 3;

        let json = serde_json::to_string(&state.player).expect("player serializes");
        let restored: PlayerState = serde_json::from_str(&json).expect("player deserializes");

        assert_eq!(restored.temp_strength, 2);
        assert_eq!(restored.temp_thorns, 4);
        assert_eq!(restored.temp_rage_block, 3);
    }
}
