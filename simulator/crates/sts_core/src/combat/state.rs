use crate::{
    action::InternalAction,
    card::CardInstance,
    content::cards::{get_card_definition, BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    content::character::IRONCLAD_A0_BASE_HP,
    content::monsters::{
        get_monster_definition, is_unsupported_approximate_monster_intent, monster_state,
        ACID_SLIME_A0, CULTIST_A0, FIXED_SIMPLE_MONSTER, GREEN_LOUSE_A0, GREMLIN_NOB_A0,
        GUARDIAN_A0, HEXAGHOST_A0, JAW_WORM_A0, LAGAVULIN_A0, LOOTER_A0, RED_LOUSE_A0, SENTRY_A0,
        SLIME_BOSS_A0, SPIKE_SLIME_A0,
    },
    ids::{CardId, MonsterId},
    power::{MonsterPowers, PlayerPowers},
    relic::{Relic, RelicCounters},
    rng::StsRng,
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
    /// In-combat zero-cost card reward from potions such as Power Potion.
    #[serde(default)]
    pub potion_card_reward: Option<Vec<CardInstance>>,
    /// Pool used by the open potion reward, retained until pick/skip because
    /// target DiscoveryAction burns additional cardRandomRng draws while the
    /// screen settles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion_card_reward_kind: Option<PotionCardRewardKind>,
    /// In-combat normal-cost colorless reward from Toolbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolbox_card_reward: Option<Vec<CardInstance>>,
    /// In-combat zero-cost card reward from Discovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_card_reward: Option<Vec<CardInstance>>,
    /// Source Discovery card waiting to move after the generated-card choice closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_source_card: Option<CardInstance>,
    /// Awaiting player choice for Warcry, Armaments, Forethought, and similar hand-select effects.
    #[serde(default)]
    pub hand_select: Option<HandSelectState>,
    /// Target actions queued behind an open hand-select screen. The action
    /// manager does not resume these until the player closes the screen.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub pending_after_hand_select_actions: VecDeque<InternalAction>,
    /// Awaiting player choice for draw-pile search effects such as Secret Technique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draw_select: Option<DrawSelectState>,
    /// Awaiting player choice for discard-pile effects such as Liquid Memories or Headbutt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard_select: Option<DiscardSelectState>,
    /// Awaiting player choice for exhaust-related effects such as Elixir, Gambling Chip, or Exhume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exhaust_select: Option<ExhaustSelectState>,
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
                no_block_turns: 0,
                vulnerable_just_applied: false,
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
            mark_of_bloom: false,
            relic_counters: RelicCounters::default(),
            ascension: 0,
            rng: CombatRngState::deterministic_fixture(0),
            potion_card_reward: None,
            potion_card_reward_kind: None,
            toolbox_card_reward: None,
            discovery_card_reward: None,
            discovery_source_card: None,
            hand_select: None,
            pending_after_hand_select_actions: VecDeque::new(),
            draw_select: None,
            discard_select: None,
            exhaust_select: None,
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
    pub fn looter_fixture() -> Self {
        let mut state = Self::initial_fixture();
        state.monsters = vec![monster_state(&LOOTER_A0, MonsterId::new(1))];
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

    /// Validates invariants required by authoritative combat transitions.
    ///
    /// This check is pure: it must not advance RNG or normalize malformed
    /// imported state into a plausible state.
    pub fn validate(&self) -> SimResult<()> {
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
        self.validate_unique_card_piles()?;
        let mut card_ids = BTreeSet::new();
        for card in self.authoritative_cards() {
            if !card_ids.insert(card.id) {
                return Err(SimError::InvalidState(
                    "duplicate authoritative card instance ID",
                ));
            }
            if get_card_definition(card.content_id).is_none() {
                return Err(SimError::UnknownContent(card.content_id));
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
                if !card_ids.insert(card.id) {
                    return Err(SimError::InvalidState(
                        "duplicate authoritative card instance ID",
                    ));
                }
                if get_card_definition(card.content_id).is_none() {
                    return Err(SimError::UnknownContent(card.content_id));
                }
            }
        }

        let active_decisions = [
            self.hand_select.is_some(),
            self.draw_select.is_some(),
            self.discard_select.is_some(),
            self.exhaust_select.is_some(),
            self.potion_card_reward.is_some(),
            self.toolbox_card_reward.is_some(),
            self.discovery_card_reward.is_some(),
        ]
        .into_iter()
        .filter(|active| *active)
        .count();
        if active_decisions > 1 {
            return Err(SimError::InvalidState(
                "multiple combat decisions are active",
            ));
        }
        if active_decisions > 0 && self.phase != CombatPhase::WaitingForPlayer {
            return Err(SimError::InvalidState(
                "combat decision is active outside the player phase",
            ));
        }
        if self.potion_card_reward.is_some() != self.potion_card_reward_kind.is_some() {
            return Err(SimError::InvalidState(
                "combat potion reward metadata is inconsistent",
            ));
        }
        if !self.pending_after_hand_select_actions.is_empty() && self.hand_select.is_none() {
            return Err(SimError::InvalidState(
                "queued hand-select actions have no active selection",
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

    fn authoritative_cards(&self) -> impl Iterator<Item = &CardInstance> {
        self.piles
            .all_cards()
            .chain(self.potion_card_reward.iter().flatten())
            .chain(self.toolbox_card_reward.iter().flatten())
            .chain(self.discovery_card_reward.iter().flatten())
            .chain(self.discovery_source_card.iter())
            .chain(
                self.discard_select
                    .iter()
                    .filter_map(|select| select.source_card.as_ref()),
            )
            .chain(
                self.exhaust_select
                    .iter()
                    .filter_map(|select| select.source_card.as_ref()),
            )
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
