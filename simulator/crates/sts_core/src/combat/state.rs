use crate::{
    action::InternalAction,
    card::CardInstance,
    content::cards::{
        get_card_definition, BASH_ID, COMBUST_DAMAGE, COMBUST_PLUS_DAMAGE, DEFEND_R_ID, STRIKE_R_ID,
    },
    content::character::IRONCLAD_A0_BASE_HP,
    content::monsters::{
        get_monster_definition, is_unsupported_approximate_monster_intent, monster_state,
        requires_rolled_attack_damage, CULTIST_A0, FIXED_SIMPLE_MONSTER, LAGAVULIN_A0,
        RED_LOUSE_A0, RED_LOUSE_BITE_DAMAGE, SENTRY_A0,
    },
    ids::{card_instance_id_is_supported, CardId, MonsterId},
    power::{MonsterPowers, PlayerPowers},
    relic::{Relic, RelicCounters},
    rng::{rng_counter_is_supported, StsRng},
    ContentId, SimError, SimResult, Snapshot, SNAPSHOT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

pub const BASE_PLAYER_ENERGY: i32 = 3;

/// Complete RNG state required by every authoritative combat.
///
/// This is flattened into `CombatState` so snapshot field names remain stable
/// while missing streams become a deserialization error instead of a runtime
/// fallback mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatRngState {
    pub shuffle_rng: StsRng,
    pub monster_rng: StsRng,
    pub monster_hp_rng: StsRng,
    pub card_random_rng: StsRng,
}

impl CombatRngState {
    /// Explicit deterministic streams for fixtures and tests.
    #[must_use]
    pub fn deterministic_fixture(seed: i64) -> Self {
        Self {
            shuffle_rng: StsRng::new(seed),
            monster_rng: StsRng::new(seed),
            monster_hp_rng: StsRng::new(seed),
            card_random_rng: StsRng::new(seed),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatState {
    pub player: PlayerState,
    pub monsters: Vec<MonsterState>,
    pub piles: CardPiles,
    pub phase: CombatPhase,
    #[serde(default)]
    pub relics: Vec<Relic>,
    /// Mark of the Bloom prevents every combat healing effect.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mark_of_bloom: bool,
    #[serde(default)]
    pub relic_counters: RelicCounters,
    #[serde(default)]
    pub ascension: u8,
    #[serde(flatten)]
    pub rng: CombatRngState,
    /// The single active player decision overlay, if combat is waiting on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<CombatDecisionState>,
    /// Later decision overlays that become active after the current one closes.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub queued_decisions: VecDeque<CombatDecisionState>,
    /// One-shot flag from Duplication Potion: the next played card resolves twice.
    #[serde(default, skip_serializing_if = "is_false")]
    pub duplication_potion_pending: bool,
    /// Remaining Duplication Potion stacks. Sacred Bark grants two stacks.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub duplication_potion_stacks: i32,
    /// Pending Double Tap stacks: the next played Attack resolves twice per stack.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub double_tap_pending: i32,
    /// Pending The Bomb explosions. Each entry ticks down at end of player turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bomb_timers: Vec<BombTimer>,
    /// Player damage queued by monster powers that add actions after card use, such as Guardian
    /// Sharp Hide.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_player_spikes_damage: i32,
    /// Energy actions queued by start-of-turn relics behind an opening combat choice.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_start_of_turn_relic_energy: i32,
    /// Monster-death relic callbacks queued behind a card effect that opened a choice screen.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub pending_monster_death_relic_triggers: u32,
    /// Gold gained by combat-only effects such as Hand of Greed before the run wrapper transfers it.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub combat_gold_gained: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BombTimer {
    pub turns_remaining: i32,
    pub damage: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PotionCardRewardKind {
    Attack,
    Skill,
    Power,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandSelectState {
    #[serde(default)]
    pub purpose: HandSelectPurpose,
    pub source_card_id: CardId,
    #[serde(default)]
    pub selected_hand_index: Option<usize>,
    #[serde(default)]
    pub selected_hand_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HandSelectPurpose {
    #[default]
    WarcryPutOnDraw,
    ArmamentsUpgrade,
    ForethoughtPutOnDraw,
    ForethoughtPutAnyOnDraw,
    ThinkingAheadPutOnDraw,
    DualWieldCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawSelectState {
    #[serde(default)]
    pub purpose: DrawSelectPurpose,
    pub source_card_id: CardId,
    pub selected_draw_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DrawSelectPurpose {
    #[default]
    SecretTechniqueSkillToHand,
    SecretWeaponAttackToHand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscardSelectState {
    #[serde(default)]
    pub purpose: DiscardSelectPurpose,
    pub source_card_id: Option<CardId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_card: Option<CardInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_discard_indices: Vec<usize>,
    #[serde(default = "default_discard_select_max_choices")]
    pub max_choices: usize,
    pub selected_discard_index: Option<usize>,
}

fn default_discard_select_max_choices() -> usize {
    1
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_card: Option<CardInstance>,
    pub selected_hand_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CombatDecisionState {
    PotionCardReward {
        choices: Vec<CardInstance>,
        reward_kind: PotionCardRewardKind,
    },
    ToolboxCardReward {
        choices: Vec<CardInstance>,
    },
    DiscoveryCardReward {
        choices: Vec<CardInstance>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_card: Option<CardInstance>,
    },
    HandSelect {
        state: HandSelectState,
        #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
        pending_actions: VecDeque<InternalAction>,
    },
    DrawSelect {
        state: DrawSelectState,
    },
    DiscardSelect {
        state: DiscardSelectState,
    },
    ExhaustSelect {
        state: ExhaustSelectState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExhaustSelectPurpose {
    #[default]
    Exhaust,
    GamblingChip,
    ExhumeReturnToHand,
    PurityExhaustUpTo3,
    BurningPactDraw2,
    BurningPactDraw3,
    TrueGritExhaustOne,
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
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub no_block_turns: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub vulnerable_just_applied: bool,
}

impl PlayerState {
    pub(crate) fn new_run_entry(hp: i32, max_hp: i32, energy_per_turn: i32) -> SimResult<Self> {
        if max_hp <= 0 || hp < 0 || hp > max_hp {
            return Err(SimError::InvalidState("combat player HP is out of bounds"));
        }
        if energy_per_turn < 0 {
            return Err(SimError::InvalidState("combat player energy is negative"));
        }
        Ok(Self {
            hp,
            max_hp,
            block: 0,
            energy: energy_per_turn,
            max_energy: energy_per_turn,
            powers: PlayerPowers::default(),
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterState {
    pub id: MonsterId,
    pub hp: i32,
    #[serde(default)]
    pub max_hp: i32,
    pub block: i32,
    pub alive: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub escaped: bool,
    pub powers: MonsterPowers,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub temp_strength_down: i32,
    pub content_id: ContentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slime_size: Option<SlimeSize>,
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
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub mode_shift_threshold: i32,
    #[serde(default)]
    pub in_defensive_mode: bool,
    #[serde(default)]
    pub rolled_attack_damage: Option<i32>,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub stolen_gold: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub move_history: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gremlin_leader_slot: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stasis_card: Option<CardInstance>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub initial_intent_locked: bool,
    pub intent: MonsterIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlimeSize {
    Small,
    Medium,
    Large,
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
    /// Encounter construction has not yet consumed this monster's initial AI
    /// roll. Authoritative combat validation rejects this transient state.
    PendingAiRoll,
    Attack {
        damage: i32,
    },
    Block {
        block: i32,
    },
    Ritual {
        amount: i32,
    },
    AttackAndBlock {
        damage: i32,
        block: i32,
    },
    StrengthAndBlock {
        strength: i32,
        block: i32,
    },
    ApplyPlayerWeak {
        amount: i32,
    },
    AttackApplyPlayerWeak {
        damage: i32,
        weak: i32,
    },
    AttackApplyPlayerVulnerable {
        damage: i32,
        vulnerable: i32,
    },
    AttackApplyPlayerWeakAndVulnerable {
        damage: i32,
        weak: i32,
        vulnerable: i32,
    },
    AttackApplyPlayerFrailAndWeak {
        damage: i32,
        frail: i32,
        weak: i32,
    },
    AttackApplyPlayerFrail {
        damage: i32,
        frail: i32,
    },
    AttackHealSelf {
        damage: i32,
    },
    ApplyPlayerHex {
        amount: i32,
    },
    ApplyPlayerFrailAndWeak {
        frail: i32,
        weak: i32,
    },
    ApplyPlayerFrailWeakVulnerable {
        frail: i32,
        weak: i32,
        vulnerable: i32,
    },
    ApplyPlayerWeakStrengthSelf {
        weak: i32,
        strength: i32,
    },
    ApplyPlayerConfusion,
    ApplyPlayerEntangled {
        amount: i32,
    },
    ApplyPlayerConstricted {
        amount: i32,
    },
    HealAllMonsters {
        amount: i32,
    },
    StrengthSelf {
        amount: i32,
    },
    StrengthAllMonsters {
        amount: i32,
    },
    EncourageGremlins {
        strength: i32,
        block: i32,
    },
    SummonGremlins {
        count: i32,
    },
    SummonCollectorTorchHeads {
        count: i32,
    },
    AttackAddWoundsToDiscard {
        damage: i32,
        count: i32,
    },
    AttackAddSlimedToDiscard {
        damage: i32,
        count: i32,
    },
    AddSlimedToDiscard {
        count: i32,
    },
    AttackStealGold {
        damage: i32,
        amount: i32,
    },
    Escape,
    Sleep,
    Stun,
    SiphonPlayer {
        strength: i32,
        dexterity: i32,
    },
    AddDazedToDiscard {
        count: i32,
    },
    AddDazedToDraw {
        count: i32,
    },
    AddBurnToDiscard {
        count: i32,
        damage: i32,
    },
    AddBurnToDiscardAndDraw {
        count: i32,
        damage: i32,
    },
    AttackMultipleUpgradeBurns {
        damage: i32,
        hits: i32,
        count: i32,
    },
    AttackMultipleApplyPlayerWeak {
        damage: i32,
        hits: i32,
        weak: i32,
    },
    AttackMultipleAddDazedToDiscard {
        damage: i32,
        hits: i32,
        count: i32,
    },
    AttackMultiple {
        damage: i32,
        hits: i32,
    },
    GuardianCloseUp {
        sharp_hide: i32,
    },
    DefensiveCharge {
        block: i32,
        strength: i32,
    },
}

impl CombatState {
    #[must_use]
    pub fn combat_card_reward_choices(&self) -> Option<&[CardInstance]> {
        match self.decision.as_ref()? {
            CombatDecisionState::PotionCardReward { choices, .. }
            | CombatDecisionState::ToolboxCardReward { choices }
            | CombatDecisionState::DiscoveryCardReward { choices, .. } => Some(choices),
            _ => None,
        }
    }

    #[must_use]
    pub fn potion_card_reward_choices(&self) -> Option<&[CardInstance]> {
        match self.decision.as_ref()? {
            CombatDecisionState::PotionCardReward { choices, .. } => Some(choices),
            _ => None,
        }
    }

    #[must_use]
    pub fn toolbox_card_reward_choices(&self) -> Option<&[CardInstance]> {
        match self.decision.as_ref()? {
            CombatDecisionState::ToolboxCardReward { choices } => Some(choices),
            _ => None,
        }
    }

    #[must_use]
    pub fn discovery_card_reward_choices(&self) -> Option<&[CardInstance]> {
        match self.decision.as_ref()? {
            CombatDecisionState::DiscoveryCardReward { choices, .. } => Some(choices),
            _ => None,
        }
    }

    #[must_use]
    pub fn hand_select(&self) -> Option<&HandSelectState> {
        match self.decision.as_ref()? {
            CombatDecisionState::HandSelect { state, .. } => Some(state),
            _ => None,
        }
    }

    pub fn hand_select_mut(&mut self) -> Option<&mut HandSelectState> {
        match self.decision.as_mut()? {
            CombatDecisionState::HandSelect { state, .. } => Some(state),
            _ => None,
        }
    }

    #[must_use]
    pub fn draw_select(&self) -> Option<&DrawSelectState> {
        match self.decision.as_ref()? {
            CombatDecisionState::DrawSelect { state } => Some(state),
            _ => None,
        }
    }

    pub fn draw_select_mut(&mut self) -> Option<&mut DrawSelectState> {
        match self.decision.as_mut()? {
            CombatDecisionState::DrawSelect { state } => Some(state),
            _ => None,
        }
    }

    #[must_use]
    pub fn discard_select(&self) -> Option<&DiscardSelectState> {
        match self.decision.as_ref()? {
            CombatDecisionState::DiscardSelect { state } => Some(state),
            _ => None,
        }
    }

    pub fn discard_select_mut(&mut self) -> Option<&mut DiscardSelectState> {
        match self.decision.as_mut()? {
            CombatDecisionState::DiscardSelect { state } => Some(state),
            _ => None,
        }
    }

    #[must_use]
    pub fn exhaust_select(&self) -> Option<&ExhaustSelectState> {
        match self.decision.as_ref()? {
            CombatDecisionState::ExhaustSelect { state } => Some(state),
            _ => None,
        }
    }

    pub fn exhaust_select_mut(&mut self) -> Option<&mut ExhaustSelectState> {
        match self.decision.as_mut()? {
            CombatDecisionState::ExhaustSelect { state } => Some(state),
            _ => None,
        }
    }

    pub fn take_hand_select(&mut self) -> Option<(HandSelectState, VecDeque<InternalAction>)> {
        match self.decision.take() {
            Some(CombatDecisionState::HandSelect {
                state,
                pending_actions,
            }) => Some((state, pending_actions)),
            other => {
                self.decision = other;
                None
            }
        }
    }

    pub fn take_draw_select(&mut self) -> Option<DrawSelectState> {
        match self.decision.take() {
            Some(CombatDecisionState::DrawSelect { state }) => Some(state),
            other => {
                self.decision = other;
                None
            }
        }
    }

    pub fn take_discard_select(&mut self) -> Option<DiscardSelectState> {
        match self.decision.take() {
            Some(CombatDecisionState::DiscardSelect { state }) => Some(state),
            other => {
                self.decision = other;
                None
            }
        }
    }

    pub fn take_exhaust_select(&mut self) -> Option<ExhaustSelectState> {
        match self.decision.take() {
            Some(CombatDecisionState::ExhaustSelect { state }) => Some(state),
            other => {
                self.decision = other;
                None
            }
        }
    }

    #[must_use]
    pub fn pending_hand_select_action_count(&self) -> usize {
        match &self.decision {
            Some(CombatDecisionState::HandSelect {
                pending_actions, ..
            }) => pending_actions.len(),
            _ => 0,
        }
    }

    pub fn activate_next_queued_decision_if_idle(&mut self) {
        if self.decision.is_none() {
            self.decision = self.queued_decisions.pop_front();
        }
    }

    pub(crate) fn new_run_entry(
        player: PlayerState,
        monsters: Vec<MonsterState>,
        piles: CardPiles,
        relics: Vec<Relic>,
        ascension: u8,
        rng: CombatRngState,
    ) -> SimResult<Self> {
        if monsters.is_empty() {
            return Err(SimError::InvalidState(
                "production combat entry requires at least one monster",
            ));
        }
        if ascension > 20 {
            return Err(SimError::InvalidState("combat ascension exceeds 20"));
        }
        let state = Self::from_entry_parts(player, monsters, piles, relics, ascension, rng);
        state.validate_unique_card_piles()?;
        Ok(state)
    }

    #[must_use]
    pub fn initial_fixture() -> Self {
        Self::from_entry_parts(
            PlayerState {
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
                no_block_turns: 0,
                vulnerable_just_applied: false,
            },
            vec![monster_state(&FIXED_SIMPLE_MONSTER, MonsterId::new(1))],
            CardPiles {
                hand: vec![
                    CardInstance::new(CardId::new(1), STRIKE_R_ID),
                    CardInstance::new(CardId::new(2), DEFEND_R_ID),
                    CardInstance::new(CardId::new(3), BASH_ID),
                ],
                draw_pile: vec![CardInstance::new(CardId::new(4), STRIKE_R_ID)],
                discard_pile: Vec::new(),
                exhaust_pile: Vec::new(),
            },
            Vec::new(),
            0,
            CombatRngState::deterministic_fixture(0),
        )
    }

    fn from_entry_parts(
        player: PlayerState,
        monsters: Vec<MonsterState>,
        piles: CardPiles,
        relics: Vec<Relic>,
        ascension: u8,
        rng: CombatRngState,
    ) -> Self {
        Self {
            player,
            monsters,
            piles,
            phase: CombatPhase::WaitingForPlayer,
            relics,
            mark_of_bloom: false,
            relic_counters: RelicCounters::default(),
            ascension,
            rng,
            decision: None,
            queued_decisions: VecDeque::new(),
            duplication_potion_pending: false,
            duplication_potion_stacks: 0,
            double_tap_pending: 0,
            bomb_timers: Vec::new(),
            pending_player_spikes_damage: 0,
            pending_start_of_turn_relic_energy: 0,
            pending_monster_death_relic_triggers: 0,
            combat_gold_gained: 0,
        }
    }

    #[must_use]
    pub fn cultist_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&CULTIST_A0, MonsterId::new(1))];
        state
    }

    #[must_use]
    pub fn red_louse_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&RED_LOUSE_A0, MonsterId::new(1))];
        state.monsters[0].rolled_attack_damage = Some(RED_LOUSE_BITE_DAMAGE);
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

    /// Validates invariants required by authoritative combat transitions.
    ///
    /// This check is pure: it must not advance RNG or normalize malformed
    /// imported state into a plausible state.
    pub fn validate(&self) -> SimResult<()> {
        if [
            self.rng.shuffle_rng.counter(),
            self.rng.monster_rng.counter(),
            self.rng.monster_hp_rng.counter(),
            self.rng.card_random_rng.counter(),
        ]
        .into_iter()
        .any(|counter| !rng_counter_is_supported(counter))
        {
            return Err(SimError::InvalidState(
                "combat RNG counter exceeds the target signed range",
            ));
        }
        if self.ascension > 20 {
            return Err(SimError::InvalidState("combat ascension exceeds 20"));
        }
        if self.player.max_hp <= 0 || self.player.hp < 0 || self.player.hp > self.player.max_hp {
            return Err(SimError::InvalidState("combat player HP is out of bounds"));
        }
        if self.player.block < 0 || self.player.energy < 0 || self.player.max_energy < 0 {
            return Err(SimError::InvalidState(
                "combat player block or energy is negative",
            ));
        }
        let combust_stacks = self.player.powers.combust;
        let combust_damage = self.player.powers.combust_damage;
        let Some(base_combust_damage) = combust_stacks.checked_mul(COMBUST_DAMAGE) else {
            return Err(SimError::InvalidState("Combust damage overflows i32"));
        };
        let Some(upgrade_bonus) = combust_damage.checked_sub(base_combust_damage) else {
            return Err(SimError::InvalidState(
                "Combust power state is inconsistent",
            ));
        };
        let damage_per_upgrade = COMBUST_PLUS_DAMAGE - COMBUST_DAMAGE;
        let Some(max_upgrade_bonus) = combust_stacks.checked_mul(damage_per_upgrade) else {
            return Err(SimError::InvalidState("Combust damage overflows i32"));
        };
        if combust_stacks < 0
            || combust_damage < 0
            || upgrade_bonus < 0
            || upgrade_bonus > max_upgrade_bonus
            || upgrade_bonus % damage_per_upgrade != 0
        {
            return Err(SimError::InvalidState(
                "Combust power state is inconsistent",
            ));
        }
        self.validate_unique_card_piles()?;
        let mut card_ids = BTreeSet::new();
        for card in self.authoritative_cards() {
            validate_combat_card(card)?;
            if !card_ids.insert(card.id) {
                return Err(SimError::InvalidState(
                    "duplicate authoritative card instance ID",
                ));
            }
        }

        let mut monster_ids = BTreeSet::new();
        for monster in &self.monsters {
            if !monster_ids.insert(monster.id) {
                return Err(SimError::InvalidState("duplicate monster instance ID"));
            }
            if get_monster_definition(monster.content_id).is_none() {
                return Err(SimError::UnknownContent(monster.content_id));
            }
            if is_unsupported_approximate_monster_intent(monster.content_id) {
                return Err(SimError::UnsupportedMechanic(monster.content_id));
            }
            if requires_rolled_attack_damage(monster.content_id)
                && monster.rolled_attack_damage.is_none()
            {
                return Err(SimError::InvalidState(
                    "monster requires rolled attack damage",
                ));
            }
            if matches!(monster.intent, MonsterIntent::PendingAiRoll) {
                return Err(SimError::InvalidState(
                    "combat monster intent is pending AI roll",
                ));
            }
            if monster.max_hp <= 0
                || monster.hp < 0
                || monster.hp > monster.max_hp
                || monster.block < 0
                || monster.stolen_gold < 0
            {
                return Err(SimError::InvalidState(
                    "combat monster HP, block, or stolen gold is out of bounds",
                ));
            }
            if let Some(card) = &monster.stasis_card {
                validate_combat_card(card)?;
                if !card_ids.insert(card.id) {
                    return Err(SimError::InvalidState(
                        "duplicate authoritative card instance ID",
                    ));
                }
            }
        }

        if (self.decision.is_some() || !self.queued_decisions.is_empty())
            && self.phase != CombatPhase::WaitingForPlayer
        {
            return Err(SimError::InvalidState(
                "combat decision is active outside the player phase",
            ));
        }
        if self.decision.is_none() && !self.queued_decisions.is_empty() {
            return Err(SimError::InvalidState(
                "queued combat decision has no active predecessor",
            ));
        }
        if self.duplication_potion_stacks < 0
            || self.double_tap_pending < 0
            || self.pending_player_spikes_damage < 0
            || self.pending_start_of_turn_relic_energy < 0
            || self.combat_gold_gained < 0
        {
            return Err(SimError::InvalidState("combat pending counter is negative"));
        }
        if self
            .bomb_timers
            .iter()
            .any(|timer| timer.turns_remaining <= 0 || timer.damage < 0)
        {
            return Err(SimError::InvalidState("combat bomb timer is invalid"));
        }

        Ok(())
    }

    fn authoritative_cards(&self) -> Vec<&CardInstance> {
        let mut cards = self.piles.all_cards().collect::<Vec<_>>();
        if let Some(decision) = &self.decision {
            extend_decision_cards(&mut cards, decision);
        }
        for decision in &self.queued_decisions {
            extend_decision_cards(&mut cards, decision);
        }
        cards
    }
}

fn validate_combat_card(card: &CardInstance) -> SimResult<()> {
    if !card_instance_id_is_supported(card.id) {
        return Err(SimError::InvalidState(
            "card instance ID is outside the supported allocation range",
        ));
    }
    if get_card_definition(card.content_id).is_none() {
        return Err(SimError::UnknownContent(card.content_id));
    }
    Ok(())
}

fn extend_decision_cards<'a>(cards: &mut Vec<&'a CardInstance>, decision: &'a CombatDecisionState) {
    match decision {
        CombatDecisionState::PotionCardReward { choices, .. }
        | CombatDecisionState::ToolboxCardReward { choices }
        | CombatDecisionState::DiscoveryCardReward { choices, .. } => cards.extend(choices),
        CombatDecisionState::DiscardSelect { state } => cards.extend(state.source_card.iter()),
        CombatDecisionState::ExhaustSelect { state } => cards.extend(state.source_card.iter()),
        CombatDecisionState::HandSelect { .. } | CombatDecisionState::DrawSelect { .. } => {}
    }
    if let CombatDecisionState::DiscoveryCardReward { source_card, .. } = decision {
        cards.extend(source_card.iter());
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

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

impl CombatState {
    /// Returns an unused card-instance ID across every authoritative combat
    /// card location, including open choices and monster stasis.
    #[must_use]
    pub fn next_card_instance_id(&self) -> u64 {
        self.authoritative_cards()
            .into_iter()
            .chain(
                self.monsters
                    .iter()
                    .filter_map(|monster| monster.stasis_card.as_ref()),
            )
            .map(|card| card.id.get())
            .max()
            .unwrap_or(0)
            + 1
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

    fn empty_piles() -> CardPiles {
        CardPiles {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
        }
    }

    #[test]
    fn production_run_entry_requires_explicit_nonempty_monsters() {
        let result = CombatState::new_run_entry(
            PlayerState::new_run_entry(42, 80, 4).expect("player state is valid"),
            Vec::new(),
            empty_piles(),
            Vec::new(),
            9,
            CombatRngState::deterministic_fixture(7),
        );

        assert_eq!(
            result,
            Err(SimError::InvalidState(
                "production combat entry requires at least one monster"
            ))
        );
    }

    #[test]
    fn production_run_entry_retains_only_caller_supplied_authoritative_state() {
        let rng = CombatRngState {
            shuffle_rng: StsRng::new(1),
            monster_rng: StsRng::new(2),
            monster_hp_rng: StsRng::new(3),
            card_random_rng: StsRng::new(4),
        };
        let monster = monster_state(&CULTIST_A0, MonsterId::new(9));

        let state = CombatState::new_run_entry(
            PlayerState::new_run_entry(42, 80, 4).expect("player state is valid"),
            vec![monster.clone()],
            empty_piles(),
            Vec::new(),
            9,
            rng.clone(),
        )
        .expect("explicit run entry is valid");

        assert_eq!(state.player.hp, 42);
        assert_eq!(state.player.max_hp, 80);
        assert_eq!(state.player.energy, 4);
        assert_eq!(state.player.max_energy, 4);
        assert_eq!(state.monsters, vec![monster]);
        assert!(state.piles.all_cards().next().is_none());
        assert_eq!(state.ascension, 9);
        assert_eq!(state.rng, rng);
    }

    #[test]
    fn combat_validation_rejects_card_ids_outside_the_allocation_domain() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand[0].id = CardId::new(0);
        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "card instance ID is outside the supported allocation range"
            ))
        );

        state.piles.hand[0].id = CardId::new(i64::MAX as u64 + 1);
        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "card instance ID is outside the supported allocation range"
            ))
        );
    }

    #[test]
    fn combat_validation_rejects_rng_counters_outside_the_target_domain() {
        let mut value =
            serde_json::to_value(CombatState::initial_fixture()).expect("combat serializes");
        value["shuffle_rng"]["counter"] = serde_json::json!(i32::MAX as u32 + 1);
        let state: CombatState = serde_json::from_value(value).expect("combat deserializes");

        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "combat RNG counter exceeds the target signed range"
            ))
        );
    }
}
