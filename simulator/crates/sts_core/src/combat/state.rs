use crate::{
    action::InternalAction,
    card::CardInstance,
    combat::cost::validate_combat_card_cost_metadata,
    content::cards::{
        get_card_definition, validate_searing_blow_metadata, BASH_ID, COMBUST_DAMAGE,
        COMBUST_PLUS_DAMAGE, DEFEND_R_ID, RAMPAGE_ID, RAMPAGE_PLUS_ID, STRIKE_R_ID,
    },
    content::character::IRONCLAD_A0_BASE_HP,
    content::monsters::{
        get_monster_definition, is_unsupported_approximate_monster_intent, monster_state,
        requires_rolled_attack_damage, CULTIST_A0, FIXED_SIMPLE_MONSTER, LAGAVULIN_A0,
        RED_LOUSE_A0, RED_LOUSE_BITE_DAMAGE, SENTRY_A0,
    },
    ids::{card_instance_id_is_supported, reserve_card_instance_id_range, CardId, MonsterId},
    potion::FAIRY_HEAL_PERCENT,
    power::{DrawTriggerPower, MonsterPowers, PlayerPowers},
    relic::{Relic, RelicCounters},
    rng::{rng_counter_is_supported, RngTraceStream, StsRng},
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
            shuffle_rng: StsRng::new(seed).for_stream(RngTraceStream::Shuffle),
            monster_rng: StsRng::new(seed).for_stream(RngTraceStream::Monster),
            monster_hp_rng: StsRng::new(seed).for_stream(RngTraceStream::MonsterHp),
            card_random_rng: StsRng::new(seed).for_stream(RngTraceStream::CardRandom),
        }
    }

    fn with_trace_streams(mut self) -> Self {
        self.shuffle_rng = self.shuffle_rng.for_stream(RngTraceStream::Shuffle);
        self.monster_rng = self.monster_rng.for_stream(RngTraceStream::Monster);
        self.monster_hp_rng = self.monster_hp_rng.for_stream(RngTraceStream::MonsterHp);
        self.card_random_rng = self.card_random_rng.for_stream(RngTraceStream::CardRandom);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatState {
    pub player: PlayerState,
    pub monsters: Vec<MonsterState>,
    pub piles: CardPiles,
    pub phase: CombatPhase,
    /// Runtime source order for player powers that enqueue draw callbacks.
    /// Scalar `PlayerPowers` values do not retain this ordering themselves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub draw_trigger_power_order: Vec<DrawTriggerPower>,
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
    /// Pen Nib: the current attack card's damage is doubled (set when the 10th
    /// attack play wraps the counter; cleared when that card finishes).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pen_nib_double_active: bool,
    /// Pending The Bomb explosions. Each entry ticks down at end of player turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bomb_timers: Vec<BombTimer>,
    /// Player damage queued by monster powers that add actions after card use, such as Guardian
    /// Sharp Hide.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_player_spikes_damage: i32,
    /// STS `AbstractPlayer.cardInUse` for the card currently resolving. Blood for
    /// Blood `tookDamage` skips this instance (FIDL00409: Pain LoseHP while BfB
    /// is mid-play must not reduce that copy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_in_use: Option<CardId>,
    /// UseCardAction applies Strange Spoon when the played card actually
    /// `moveToExhaustPile`s, after that card's `addToBot` effects. Violence
    /// builds its attack tmp group with `cardRandomRng` in ViolenceAction,
    /// which runs before settlement (FIDL01427).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer_strange_spoon_until_source_move: Option<CardId>,
    /// Set while expanding `PlayTopDrawCard` with `exhaust_played_card` so nested
    /// Dual Wield select knows to force-exhaust on CONFIRM (no exhaust keyword).
    #[serde(default, skip_serializing_if = "is_false")]
    pub play_top_force_exhaust_active: bool,
    /// Verifier-only: PutOnDeckAction completed before its auto-place update,
    /// so a singleton Warcry draw stays in hand instead of returning to draw.
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip_put_on_deck_auto_place: bool,
    /// Malleable/Curl Up GainMonsterBlock from nested PlayTop attacks, flushed
    /// after the outer skill's bot actions (Letter Opener) — FIDL00428.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred_play_top_monster_blocks: Vec<(crate::ids::MonsterId, i32)>,
    /// Depth while expanding nested PlayTop card queues (not sticky across turns).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub play_top_resolving_depth: u32,
    /// Start-of-turn Mayhem PlayTop `MakeTempCardInDrawPile` waits behind Evolve
    /// residual draws from the base hand refill (FIDL01469 Wild Strike Wound).
    #[serde(default, skip_serializing_if = "is_false")]
    pub defer_mayhem_play_top_draw_inserts: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred_mayhem_play_top_draw_inserts: Vec<InternalAction>,
    /// Letter Opener all-enemy hits still on the action queue (FIDL00428).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub pending_letter_opener_blasts: u32,
    /// Event combat's opening action queue has not yet reached the player turn.
    /// The target publishes the newly entered event combat before its initial
    /// DrawCardAction and initial monster AI actions settle.
    #[serde(default, skip_serializing_if = "is_false")]
    pub opening_turn_pending: bool,
    /// Initial monster intents already chosen by the queued event-combat setup.
    /// They are restored when the opening END drains that queue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_opening_monster_intents: Vec<MonsterIntent>,
    /// The opening END remains queued behind the first player action after its
    /// initial draw publication.
    #[serde(default, skip_serializing_if = "is_false")]
    pub opening_end_turn_pending: bool,
    /// Colosseum fight-two leftover EndTurn already ran callEndOfTurnActions
    /// before the ready PLAY. Flex applied on that frame must survive the
    /// following start_player_turn (FIDL01576). Other leftover ends still
    /// expire temp strength at the next start.
    #[serde(default, skip_serializing_if = "is_false")]
    pub preserve_temp_strength_on_next_start: bool,
    /// Opening DrawCardAction parked behind a first-turn Toolbox choice.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub pending_opening_hand_draw: usize,
    /// Anchor's opening GainBlockAction parked behind a first-turn Toolbox choice.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_opening_combat_block: i32,
    /// Energy actions queued by start-of-turn relics behind an opening combat choice.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_start_of_turn_relic_energy: i32,
    /// Mercury Hourglass damage queued behind a first-turn Toolbox choice.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_start_of_turn_relic_damage: i32,
    /// Monster-death relic callbacks queued behind a card effect that opened a choice screen.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub pending_monster_death_relic_triggers: u32,
    /// Gold gained by combat-only effects such as Hand of Greed before the run wrapper transfers it.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub combat_gold_gained: i32,
    /// Set only when Writhing Mass's Mega Debuff intent actually executes;
    /// intent selection alone must not publish its queued Parasite.
    #[serde(default, skip_serializing_if = "is_false")]
    pub writhing_mass_mega_debuff_triggered: bool,
    /// Evolve/Fire Breathing callbacks from an end-turn HP-loss DrawCardAction
    /// stay behind the bulk hand discard in the source action queue.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub pending_hp_loss_draw_follow_ups: VecDeque<InternalAction>,
    /// Deferred DiscoveryAction generations after a potion reward selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_potion_card_reward_settlement: Option<PendingPotionCardRewardSettlement>,
    /// Leftover HandCardSelectScreen.selectedCards held off every serialized
    /// pile until a non-empty-hand end-turn DiscardAction (skipped retrieval).
    /// Covers Cultist-potion interleaving, Dual Wield / Armaments / Burning Pact
    /// skipped retrieval, and multi-card Elixir / Gambling Chip selects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_hidden_hand_card_until_end_turn: Vec<CardInstance>,
    /// A deferred Burning Pact selection under Runic Pyramid remains hidden
    /// through END and is reclaimed by Fiend Fire's exhaust-all action.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pending_hidden_hand_card_exhausts_with_fiend_fire: bool,
    /// Set when Nilry's Codex paused end-of-turn; resume discards + monster turn
    /// after the card-reward decision closes.
    ///
    /// CommunicationMod publishes a two-step Codex end-turn (FIDL00451):
    /// END → first 3-card offer (hand still held) → CHOOSE/SKIP → END →
    /// second 3-card offer (hand discarded) → CHOOSE/SKIP → monster/draw.
    #[serde(default, skip_serializing_if = "is_false")]
    pub resume_end_turn_after_nilrys_codex: bool,
    /// Nilry two-offer end-turn stage (FIDL00451):
    /// 0 inactive, 1 first offer open, 2 await END for second offer,
    /// 3 second offer open.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub nilrys_codex_end_turn_stage: u8,
    /// Combust / other `atEndOfTurn` powers wait behind the first Codex offer
    /// because `onPlayerEndTurn` queues Nilry before those powers (FIDL01727).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_end_powers_pending: bool,
    /// Cards chosen from Nilry offers this end-turn; inserted into the draw
    /// pile only when end-turn finally resumes (FIDL00451 first pick stays out
    /// of the draw pile during the second offer frame).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_nilrys_codex_draw_inserts: Vec<crate::ContentId>,
    /// Dead Branch cards held across the Nilry pause until post-discard hand rebuild.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_end_turn_dead_branch_cards: Vec<CardInstance>,
    /// Dark Embrace draws deferred across the Nilry pause.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub pending_end_turn_dark_embrace_draws: usize,
    /// Juggernaut damage queued by end-turn ethereal exhaust until monster
    /// pre-turn block clearing has run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_end_turn_juggernaut_damage: Vec<i32>,
    /// Legacy fields retained for snapshot deserialization compatibility.
    /// Elixir permanently exhausts selected cards; these are no longer written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_elixir_exhaust_card_ids: Vec<CardId>,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub pending_elixir_exhaust_turns_remaining: u8,
    /// Time Eater ends the current player turn after the twelfth card resolves.
    #[serde(default, skip_serializing_if = "is_false")]
    pub time_warp_end_turn: bool,
    /// The Time Warp lag frame already resolved end-of-player-turn powers and
    /// auto-playing hand cards; the next forced-turn settlement starts at
    /// ethereal exhaustion/discard rather than replaying that queue.
    #[serde(default, skip_serializing_if = "is_false")]
    pub time_warp_end_turn_pre_discard_settled: bool,
    /// Metallicize / other pre-card end-turn powers already ran on a Time Warp
    /// exhaust-select lag frame; the forced END must not grant them again.
    #[serde(default, skip_serializing_if = "is_false")]
    pub time_warp_end_powers_applied: bool,
    /// The source action manager has two monster queue entries pending after a
    /// Time Warp hand/exhaust-select lag frame. Each entry captured the same
    /// intent before its RollMoveAction ran.
    #[serde(default, skip_serializing_if = "is_false")]
    pub time_warp_duplicate_monster_queue: bool,
    /// Two-step Nilry `END` leaves the first `EndTurnAction` queued while a
    /// second `EndTurnAction` opens the second Codex. Both
    /// `MonsterQueueItem`s are captured before either `RollMoveAction`
    /// (FIDL01772 / FIDL01727), the same multiplicity as Time Warp.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_duplicate_monster_queue: bool,
    /// Second Book multi-stab takeTurn reads the live StabCount after the
    /// first queued take (FIDL01727 step 887: 7+8). Captured N+N stays the
    /// default (step 880: 6+6). StabCount still advances only in `getMove`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_book_second_stab_uses_live_count: bool,
    /// SuperFastMode can publish the stage-3 close after duplicate takeTurns
    /// but before StrengthSelf RollMoveActions (FIDL01486 Byrd stays Caw).
    /// Attackers still consume both leftover rolls.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_hold_strength_self_rolls: bool,
    /// SuperFastMode can publish after StrengthSelf's first leftover roll
    /// and before other monsters' RollMoveActions (FIDL01486 SKIP 468:
    /// Byrd Peck, Chosen still Drain).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_one_strength_self_roll_hold_others: bool,
    /// Leftover RollMoveActions can run Byrd, Chosen, Byrd, Chosen instead of
    /// both Byrd rolls first (FIDL01486 CHOOSE 478: Caw then Debilitate).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_interleave_post_queue_rolls: bool,
    /// SuperFastMode can publish Peck without applying leftover Byrd
    /// RollMoveActions, so Chosen consumes those `monster_rng` draws
    /// (FIDL01486 SKIP 491: Peck stays, Chosen rolls Drain).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_hold_attack_multiple_rolls: bool,
    /// SuperFastMode can publish after the first leftover RollMoveAction
    /// (FIDL01727 Collector Mega Debuff, not the second roll's Fireball).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_single_post_queue_roll: bool,
    /// SuperFastMode can finish leftover takeTurns and the next player draw
    /// before any leftover RollMoveAction (FIDL01727 CHOOSE 1059: Collector
    /// stays Fireball; the Buff / Spawn rolls publish on later closes).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_skip_post_queue_rolls: bool,
    /// SuperFastMode leftover stage-3 Codex close can run `DrawCardAction` and
    /// Warped Tongs before `MakeTempCardInDrawPileAction` (FIDL01807 CHOOSE
    /// 1167: Strike is drawn; the chosen Codex card lands in the remaining
    /// draw pile). Default insert-before-draw displaces that last deck card.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_defer_codex_insert_until_after_draw: bool,
    /// SuperFastMode leftover Dual Wield / first-offer Codex close can keep
    /// MakeTempCardInDrawPile occupancy, so addToRandomSpot rolls the same
    /// bound more than once (FIDL01807 CHOOSE 949: Intimidate at draw index 12).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub nilrys_codex_insert_same_bound_rolls: u8,
    /// First-offer leftover Codex insert can consume `shuffleRng` instead of
    /// `cardRandomRng` (FIDL01807 CHOOSE 949 Intimidate at draw index 12).
    #[serde(default, skip_serializing_if = "is_false")]
    pub nilrys_codex_insert_uses_shuffle_rng: bool,
    /// Feel No Pain / other end-turn exhaust block granted while leftover
    /// EndTurn is still flushing. The first leftover STATE can publish the
    /// discarded hand before that GainBlockAction (FIDL01727 step 821).
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pending_end_turn_feel_no_pain_block: i32,
    /// The first forced Time Warp END can publish after monster turn setup and
    /// before its captured attack action; the next END resumes that action.
    #[serde(default, skip_serializing_if = "is_false")]
    pub time_warp_pending_monster_action: bool,
    /// Start-of-turn queued draws must finish before a Time Warp forced end-turn.
    /// This mirrors GameActionManager's FIFO: Mayhem resolves, then Evolve's
    /// DrawCardAction, then the EndTurnAction appended by Time Warp.
    #[serde(default, skip_serializing_if = "is_false")]
    pub defer_time_warp_end_turn: bool,
    /// SuperFastMode leftover EndTurn published after EmptyDeckShuffleAction
    /// and before the remaining DrawCardAction cards (FIDL01691 STATE 1352).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub leftover_end_turn_draw_remaining: u8,
    /// DiscoveryAction retrieved a card this combat. SuperFastMode leftover
    /// MakeTempCardInDrawPile occupancy after that retrieve lasts into later
    /// Reckless Charge Dazed inserts versus Time Eater (FIDL01680).
    #[serde(default, skip_serializing_if = "is_false")]
    pub discovery_retrieved_this_combat: bool,
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

/// DiscoveryAction choice generations that remain after a potion reward is
/// selected. CommunicationMod can accept combat commands while that action is
/// still settling, so its lifecycle is authoritative simulator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPotionCardRewardSettlement {
    pub reward_kind: PotionCardRewardKind,
    pub generations_remaining: u32,
    pub end_turns_remaining: u8,
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
    /// Force-play Dual Wield multi-select hides non-Attack/Power cards from the
    /// CommunicationMod hand. Skills (Defend, etc.) re-enter hand on CONFIRM
    /// (random-fidelity-9074); statuses/curses stay out of every combat pile for
    /// the rest of the fight (FIDL00242 Shame).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dual_wield_restore_on_confirm: Vec<CardInstance>,
    /// Havoc/Mayhem/Distilled Chaos force-play exhausts Dual Wield even though the
    /// card definition has no exhaust keyword (hand-play discards; FIDL00242 vs
    /// trace-session-8).
    #[serde(default, skip_serializing_if = "is_false")]
    pub dual_wield_force_exhaust: bool,
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
    /// The target grid's CardGroup.addToRandomSpot order, captured when the
    /// draw-selection screen is opened.
    #[serde(default)]
    pub selectable_card_ids: Vec<CardId>,
    pub selected_draw_index: Option<usize>,
    /// Follow-ups queued after the draw-selection screen opens. CommunicationMod
    /// can publish the grid before on-use generated cards resolve, so preserve
    /// them until CONFIRM rather than mutating the visible draw pile early.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub pending_actions: VecDeque<InternalAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DrawSelectPurpose {
    #[default]
    SecretTechniqueSkillToHand,
    SecretWeaponAttackToHand,
    Scry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscardSelectState {
    #[serde(default)]
    pub purpose: DiscardSelectPurpose,
    pub source_card_id: Option<CardId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_card: Option<CardInstance>,
    /// Force-played Headbutt (Havoc / Mayhem / Distilled Chaos) parks this so
    /// the global play-top marker cannot leak onto a later hand play.
    #[serde(default, skip_serializing_if = "is_false")]
    pub source_card_force_exhaust: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_discard_indices: Vec<usize>,
    #[serde(default = "default_discard_select_max_choices")]
    pub max_choices: usize,
    pub selected_discard_index: Option<usize>,
    /// Actions queued behind a Headbutt/Liquid Memories discard grid. The
    /// target keeps these behind the screen until CONFIRM closes it.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub pending_actions: VecDeque<InternalAction>,
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
    /// Force-played sources (Havoc, Mayhem, or Distilled Chaos) settle with
    /// exhaust-on-use semantics after the selection closes. Ordinary hand-play
    /// True Grit+ parks its source here too, but that source discards.
    #[serde(default, skip_serializing_if = "is_false")]
    pub source_card_force_exhaust: bool,
    pub selected_hand_indices: Vec<usize>,
    /// Cultist Potion can be used while Burning Pact is waiting for an
    /// exhaust selection. The target action queue leaves the selected card
    /// pending in that specific interleaving instead of exhausting it.
    #[serde(default)]
    pub interrupted_by_cultist_potion: bool,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    pub pending_actions: VecDeque<InternalAction>,
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
        /// A forced PlayTop source settles with exhaustOnUseOnce after the
        /// reward closes, even when the card itself has no Exhaust keyword.
        #[serde(default, skip_serializing_if = "is_false")]
        source_card_force_exhaust: bool,
        /// Mayhem PlayTop opens Discovery without force-exhaust. SuperFastMode
        /// leftover settlement drains extra `DiscoveryAction` pulses before
        /// CHOOSE, so Magnetism's two-pulse early-turn retrieve does not apply
        /// (FIDL01255 Deep Breath / Good Instincts).
        #[serde(default, skip_serializing_if = "is_false")]
        source_card_play_top: bool,
        /// Hex/onUseCard bot follow-ups that must wait until the reward closes
        /// (FIDL00233: Hex Dazed lands on Discovery CHOOSE, not on PLAY open).
        #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
        pending_actions: VecDeque<InternalAction>,
    },
    /// Nilry's Codex end-of-turn offer: pick one card to shuffle into the draw
    /// pile (or skip). End-turn continues after the decision closes.
    NilrysCodexCardReward {
        choices: Vec<CardInstance>,
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
    RecycleExhaustOne,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    #[serde(default = "default_player_energy")]
    pub max_energy: i32,
    /// Energy granted by effects such as Charge Battery at the next turn start.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub energy_next_turn: i32,
    /// Equilibrium retains the current hand through the next end-turn discard.
    #[serde(default, skip_serializing_if = "is_false")]
    pub retain_hand_next_turn: bool,
    /// Target AbstractPlayer.damagedThisCombat: positive damage/loss events
    /// delivered during this combat, used when generated cards are copied.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub damage_events_this_combat: i32,
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
    /// WeakPower.atEndOfRound skips one decrement after a monster apply.
    #[serde(default, skip_serializing_if = "is_false")]
    pub weak_just_applied: bool,
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
            energy_next_turn: 0,
            retain_hand_next_turn: false,
            damage_events_this_combat: 0,
            powers: PlayerPowers::default(),
            cannot_draw: false,
            temp_strength: 0,
            temp_dexterity: 0,
            temp_thorns: 0,
            temp_rage_block: 0,
            no_block_turns: 0,
            vulnerable_just_applied: false,
            weak_just_applied: false,
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
    #[serde(default, skip_serializing_if = "is_false")]
    pub vulnerable_just_applied: bool,
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
    /// A survived Hexaghost Inferno makes later Sear-generated Burns upgraded.
    #[serde(default, skip_serializing_if = "is_false")]
    pub burns_upgraded: bool,
    /// When set, the next half-dead REBIRTH takeTurn only holds the
    /// half-dead pose; heal + Dark Echo wait for the following monster turn.
    /// Set when first death lands during player end-of-turn powers (Combust),
    /// matching FIDL00391 (death END stays half-dead through one player turn).
    #[serde(default, skip_serializing_if = "is_false")]
    pub defer_awakened_one_rebirth: bool,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limbo: Vec<CardInstance>,
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
    /// Darkling's half-dead COUNT pose before its next REINCARNATE turn.
    /// The target exposes this as `Intent.NONE` / UNKNOWN with move byte 4.
    DarklingCount,
    /// Awakened One's first-death REBIRTH pose before its next monster turn.
    /// The target sets move byte 3 while exposing `Intent.UNKNOWN`.
    AwakenedOneHalfDead,
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
    AttackApplyPlayerFrailAndVulnerable {
        damage: i32,
        frail: i32,
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
    AttackAddVoidToDraw {
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
    /// Record a source power entering or leaving the active draw-callback list.
    pub(crate) fn update_draw_trigger_power_order(
        &mut self,
        power: DrawTriggerPower,
        was_active: bool,
        is_active: bool,
    ) {
        if is_active {
            if !was_active && !self.draw_trigger_power_order.contains(&power) {
                self.draw_trigger_power_order.push(power);
            }
        } else {
            self.draw_trigger_power_order
                .retain(|candidate| *candidate != power);
        }
    }

    /// Return active draw-trigger powers in their authoritative source order.
    /// Missing order is an invalid state when both callbacks are active; a
    /// deterministic lexical fallback would silently weaken replay fidelity.
    pub(crate) fn active_draw_trigger_powers(&self) -> SimResult<Vec<DrawTriggerPower>> {
        let active = [
            (DrawTriggerPower::Evolve, self.player.powers.evolve > 0),
            (
                DrawTriggerPower::FireBreathing,
                self.player.powers.fire_breathing > 0,
            ),
        ];
        let active_count = active.iter().filter(|(_, is_active)| *is_active).count();
        if active_count == 0 {
            return Ok(Vec::new());
        }
        let mut ordered = Vec::with_capacity(active_count);
        for power in &self.draw_trigger_power_order {
            if active
                .iter()
                .any(|(candidate, is_active)| candidate == power && *is_active)
                && !ordered.contains(power)
            {
                ordered.push(*power);
            }
        }
        if ordered.len() != active_count {
            if active_count == 1 {
                // With only one active callback there is no ordering question;
                // this also keeps compact unit fixtures that set one scalar
                // power directly semantically complete.
                return Ok(active
                    .iter()
                    .find_map(|(power, is_active)| is_active.then_some(*power))
                    .into_iter()
                    .collect());
            }
            return Err(SimError::InvalidState(
                "active draw-trigger power order is incomplete",
            ));
        }
        Ok(ordered)
    }

    #[must_use]
    pub fn combat_card_reward_choices(&self) -> Option<&[CardInstance]> {
        match self.decision.as_ref()? {
            CombatDecisionState::PotionCardReward { choices, .. }
            | CombatDecisionState::ToolboxCardReward { choices }
            | CombatDecisionState::DiscoveryCardReward { choices, .. }
            | CombatDecisionState::NilrysCodexCardReward { choices } => Some(choices),
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

    /// Drop open hand/exhaust selects when combat is already over (FIDL00243:
    /// lethal play can leave a stale decision while phase is Won).
    pub fn clear_decisions_on_combat_end(&mut self) {
        if matches!(self.phase, CombatPhase::Won | CombatPhase::Lost) {
            self.decision = None;
            self.queued_decisions.clear();
        }
    }

    pub(crate) fn queue_or_activate_decision(&mut self, decision: CombatDecisionState) {
        if self.decision.is_some() {
            self.queued_decisions.push_back(decision);
        } else {
            self.decision = Some(decision);
        }
    }

    /// Park the opening draw behind a first-turn Toolbox choice.
    pub(crate) fn defer_opening_hand_draw(&mut self) {
        if self.pending_opening_hand_draw > 0 {
            return;
        }
        let opening_hand = std::mem::take(&mut self.piles.hand);
        self.pending_opening_hand_draw = opening_hand.len();
        self.piles.draw_pile.extend(opening_hand.into_iter().rev());
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
        let state = Self::from_entry_parts(
            player,
            monsters,
            piles,
            relics,
            ascension,
            rng.with_trace_streams(),
        );
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
                energy_next_turn: 0,
                retain_hand_next_turn: false,
                damage_events_this_combat: 0,
                powers: PlayerPowers::default(),
                cannot_draw: false,
                temp_strength: 0,
                temp_dexterity: 0,
                temp_thorns: 0,
                temp_rage_block: 0,
                no_block_turns: 0,
                vulnerable_just_applied: false,
                weak_just_applied: false,
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
                limbo: Vec::new(),
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
            draw_trigger_power_order: Vec::new(),
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
            pen_nib_double_active: false,
            bomb_timers: Vec::new(),
            pending_player_spikes_damage: 0,
            card_in_use: None,
            defer_strange_spoon_until_source_move: None,
            play_top_force_exhaust_active: false,
            skip_put_on_deck_auto_place: false,
            deferred_play_top_monster_blocks: Vec::new(),
            play_top_resolving_depth: 0,
            defer_mayhem_play_top_draw_inserts: false,
            deferred_mayhem_play_top_draw_inserts: Vec::new(),
            pending_letter_opener_blasts: 0,
            opening_turn_pending: false,
            pending_opening_monster_intents: Vec::new(),
            opening_end_turn_pending: false,
            preserve_temp_strength_on_next_start: false,
            pending_opening_hand_draw: 0,
            pending_opening_combat_block: 0,
            pending_start_of_turn_relic_energy: 0,
            pending_start_of_turn_relic_damage: 0,
            pending_monster_death_relic_triggers: 0,
            combat_gold_gained: 0,
            pending_hp_loss_draw_follow_ups: VecDeque::new(),
            writhing_mass_mega_debuff_triggered: false,
            pending_potion_card_reward_settlement: None,
            pending_hidden_hand_card_until_end_turn: Vec::new(),
            pending_hidden_hand_card_exhausts_with_fiend_fire: false,
            resume_end_turn_after_nilrys_codex: false,
            nilrys_codex_end_turn_stage: 0,
            nilrys_end_powers_pending: false,
            pending_nilrys_codex_draw_inserts: Vec::new(),
            pending_end_turn_dead_branch_cards: Vec::new(),
            pending_end_turn_dark_embrace_draws: 0,
            pending_end_turn_juggernaut_damage: Vec::new(),
            pending_elixir_exhaust_card_ids: Vec::new(),
            pending_elixir_exhaust_turns_remaining: 0,
            time_warp_end_turn: false,
            time_warp_end_turn_pre_discard_settled: false,
            time_warp_end_powers_applied: false,
            time_warp_duplicate_monster_queue: false,
            nilrys_duplicate_monster_queue: false,
            nilrys_book_second_stab_uses_live_count: false,
            nilrys_hold_strength_self_rolls: false,
            nilrys_one_strength_self_roll_hold_others: false,
            nilrys_interleave_post_queue_rolls: false,
            nilrys_hold_attack_multiple_rolls: false,
            nilrys_single_post_queue_roll: false,
            nilrys_skip_post_queue_rolls: false,
            nilrys_defer_codex_insert_until_after_draw: false,
            nilrys_codex_insert_same_bound_rolls: 0,
            nilrys_codex_insert_uses_shuffle_rng: false,
            pending_end_turn_feel_no_pain_block: 0,
            time_warp_pending_monster_action: false,
            defer_time_warp_end_turn: false,
            leftover_end_turn_draw_remaining: 0,
            discovery_retrieved_this_combat: false,
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
        let mut decision_cards = Vec::new();
        if let Some(decision) = &self.decision {
            extend_decision_cards(&mut decision_cards, decision);
        }
        for decision in &self.queued_decisions {
            extend_decision_cards(&mut decision_cards, decision);
        }
        for card in self
            .piles
            .all_cards()
            .chain(self.pending_hidden_hand_card_until_end_turn.iter())
            .chain(decision_cards)
        {
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
        if self.relic_counters.has_out_of_bounds_stable_counter() {
            return Err(SimError::InvalidState(
                "combat relic counter is outside its stable range",
            ));
        }
        let fairy_heal_percent = self.relic_counters.fairy_heal_percent;
        if fairy_heal_percent != 0
            && fairy_heal_percent != FAIRY_HEAL_PERCENT
            && fairy_heal_percent != FAIRY_HEAL_PERCENT * 2
        {
            return Err(SimError::InvalidState(
                "combat fairy revival percentage is outside the target domain",
            ));
        }
        if self.relic_counters.fairy_consumed && fairy_heal_percent != 0 {
            return Err(SimError::InvalidState(
                "consumed combat fairy retains a revival percentage",
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
        if self.player.damage_events_this_combat < 0 {
            return Err(SimError::InvalidState(
                "combat player damage counter is negative",
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
            if monster.burns_upgraded
                && monster.content_id != crate::content::monsters::HEXAGHOST_ID
            {
                return Err(SimError::InvalidState(
                    "non-Hexaghost monster carries upgraded Burn generation",
                ));
            }
            if matches!(monster.intent, MonsterIntent::PendingAiRoll) && !self.opening_turn_pending
            {
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

        if self.opening_turn_pending
            && self.pending_opening_monster_intents.len() != self.monsters.len()
        {
            return Err(SimError::InvalidState(
                "opening combat intent queue does not match monsters",
            ));
        }
        if !self.opening_turn_pending && !self.pending_opening_monster_intents.is_empty() {
            return Err(SimError::InvalidState("stale opening combat intent queue"));
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
            || self.pending_opening_combat_block < 0
            || self.pending_start_of_turn_relic_energy < 0
            || self.pending_start_of_turn_relic_damage < 0
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
        if let Some(pending) = self.pending_potion_card_reward_settlement {
            if pending.generations_remaining == 0 || pending.end_turns_remaining == 0 {
                return Err(SimError::InvalidState(
                    "pending potion card reward settlement is empty",
                ));
            }
            // DiscoveryAction remains on the action queue while CommunicationMod
            // can expose a later card decision; the decision overlay is therefore
            // allowed to coexist with this deferred internal lifecycle.
        }

        Ok(())
    }

    fn authoritative_cards(&self) -> Vec<&CardInstance> {
        let mut cards = self.piles.all_cards().collect::<Vec<_>>();
        // Skipped-retrieval / deferred exhaust selections park cards here until
        // END. They must reserve instance IDs so generated wounds (Wild Strike)
        // cannot collide and later fail unique-pile validation (FIDL00222).
        cards.extend(self.pending_hidden_hand_card_until_end_turn.iter());
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
    if get_card_definition(card.content_id).is_none()
        && crate::run::reward::any_color_reward_card_key(card.content_id).is_none()
    {
        return Err(SimError::UnknownContent(card.content_id));
    }
    validate_searing_blow_metadata(card)?;
    validate_combat_card_cost_metadata(card)?;
    if card.rampage_damage_bonus < 0 {
        return Err(SimError::InvalidState(
            "Rampage damage bonus cannot be negative",
        ));
    }
    if card.rampage_damage_bonus != 0
        && card.content_id != RAMPAGE_ID
        && card.content_id != RAMPAGE_PLUS_ID
    {
        return Err(SimError::InvalidState(
            "non-Rampage card carries a Rampage damage bonus",
        ));
    }
    Ok(())
}

fn extend_decision_cards<'a>(cards: &mut Vec<&'a CardInstance>, decision: &'a CombatDecisionState) {
    match decision {
        CombatDecisionState::PotionCardReward { choices, .. }
        | CombatDecisionState::ToolboxCardReward { choices }
        | CombatDecisionState::DiscoveryCardReward { choices, .. }
        | CombatDecisionState::NilrysCodexCardReward { choices } => cards.extend(choices),
        CombatDecisionState::DiscardSelect { state } => cards.extend(state.source_card.iter()),
        CombatDecisionState::ExhaustSelect { state } => cards.extend(state.source_card.iter()),
        CombatDecisionState::HandSelect { state, .. } => {
            cards.extend(state.dual_wield_restore_on_confirm.iter());
        }
        CombatDecisionState::DrawSelect { .. } => {}
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

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

impl CombatState {
    /// Returns the greatest card-instance ID across every authoritative combat
    /// card location, including open choices and monster stasis.
    #[must_use]
    pub(crate) fn max_authoritative_card_instance_id(&self) -> u64 {
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
    }

    /// Reserves a contiguous range after every authoritative combat card
    /// location, including open choices and monster stasis.
    pub(crate) fn reserve_card_instance_ids(&self, count: usize) -> SimResult<u64> {
        reserve_card_instance_id_range(self.max_authoritative_card_instance_id(), count)
    }

    /// Returns one checked unused card-instance ID across every authoritative
    /// combat card location, including open choices and monster stasis.
    pub fn next_card_instance_id(&self) -> SimResult<u64> {
        self.reserve_card_instance_ids(1)
    }
}

impl CardPiles {
    pub(crate) fn max_card_instance_id(&self) -> u64 {
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
            .chain(self.limbo.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{SEARING_BLOW_ID, SEARING_BLOW_PLUS_ID};

    fn empty_piles() -> CardPiles {
        CardPiles {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            limbo: Vec::new(),
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
    fn combat_validation_rejects_invalid_rampage_metadata() {
        let mut negative = CombatState::initial_fixture();
        negative.piles.hand = vec![CardInstance::new(CardId::new(100), RAMPAGE_ID)];
        negative.piles.hand[0].rampage_damage_bonus = -1;
        assert_eq!(
            negative.validate(),
            Err(SimError::InvalidState(
                "Rampage damage bonus cannot be negative"
            ))
        );

        let mut wrong_card = CombatState::initial_fixture();
        wrong_card.piles.hand[0].rampage_damage_bonus = 5;
        assert_eq!(
            wrong_card.validate(),
            Err(SimError::InvalidState(
                "non-Rampage card carries a Rampage damage bonus"
            ))
        );
    }

    #[test]
    fn combat_validation_rejects_upgraded_burn_generation_on_non_hexaghost() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].burns_upgraded = true;

        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "non-Hexaghost monster carries upgraded Burn generation"
            ))
        );
    }

    #[test]
    fn combat_validation_rejects_inconsistent_searing_blow_metadata() {
        let mut base = CombatState::initial_fixture();
        base.piles.hand = vec![CardInstance::new(CardId::new(100), SEARING_BLOW_ID)];
        base.piles.hand[0].searing_blow_upgrades = 1;
        assert_eq!(
            base.validate(),
            Err(SimError::InvalidState(
                "base Searing Blow carries upgrade-count metadata"
            ))
        );

        let mut upgraded = CombatState::initial_fixture();
        upgraded.piles.hand = vec![CardInstance::new(CardId::new(100), SEARING_BLOW_PLUS_ID)];
        assert_eq!(
            upgraded.validate(),
            Err(SimError::InvalidState(
                "Searing Blow+ is missing its upgrade count"
            ))
        );

        let mut wrong_card = CombatState::initial_fixture();
        wrong_card.piles.hand[0].searing_blow_upgrades = 1;
        assert_eq!(
            wrong_card.validate(),
            Err(SimError::InvalidState(
                "non-Searing-Blow card carries Searing Blow upgrade metadata"
            ))
        );
    }

    #[test]
    fn combat_validation_rejects_invalid_card_cost_metadata() {
        let mut negative = CombatState::initial_fixture();
        negative.piles.hand = vec![CardInstance::new(
            CardId::new(100),
            crate::content::cards::BLOOD_FOR_BLOOD_ID,
        )];
        negative.piles.hand[0].blood_for_blood_cost_reduction = -1;
        assert_eq!(
            negative.validate(),
            Err(SimError::InvalidState(
                "Blood for Blood cost reduction cannot be negative"
            ))
        );

        let mut wrong_card = CombatState::initial_fixture();
        wrong_card.piles.hand[0].blood_for_blood_cost_reduction = 1;
        assert_eq!(
            wrong_card.validate(),
            Err(SimError::InvalidState(
                "non-Blood-for-Blood card carries cost-reduction metadata"
            ))
        );

        let mut missing_temp_cost = CombatState::initial_fixture();
        missing_temp_cost.piles.hand[0].temp_cost_turn_only = true;
        assert_eq!(
            missing_temp_cost.validate(),
            Err(SimError::InvalidState(
                "turn-only card cost flag has no temporary cost"
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

    #[test]
    fn combat_validation_rejects_consumed_relic_counter_values() {
        let mut state = CombatState::initial_fixture();
        state.relic_counters.ink_bottle_cards_played = crate::relic::INK_BOTTLE_THRESHOLD - 1;
        state.validate().expect("last unconsumed counter is valid");

        state.relic_counters.ink_bottle_cards_played = crate::relic::INK_BOTTLE_THRESHOLD;
        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "combat relic counter is outside its stable range"
            ))
        );
    }

    #[test]
    fn combat_validation_rejects_relic_counters_outside_the_target_domain() {
        let mut state = CombatState::initial_fixture();
        state.relic_counters.player_turns_started = i32::MAX as u32;
        state.validate().expect("largest target counter is valid");

        state.relic_counters.player_turns_started = i32::MAX as u32 + 1;
        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "combat relic counter is outside its stable range"
            ))
        );
    }

    #[test]
    fn combat_validation_rejects_impossible_fairy_revival_state() {
        let mut state = CombatState::initial_fixture();
        state.relic_counters.fairy_heal_percent = FAIRY_HEAL_PERCENT;
        state.validate().expect("ordinary Fairy is valid");
        state.relic_counters.fairy_heal_percent = FAIRY_HEAL_PERCENT * 2;
        state.validate().expect("Sacred Bark Fairy is valid");

        state.relic_counters.fairy_heal_percent = FAIRY_HEAL_PERCENT + 1;
        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "combat fairy revival percentage is outside the target domain"
            ))
        );

        state.relic_counters.fairy_heal_percent = FAIRY_HEAL_PERCENT;
        state.relic_counters.fairy_consumed = true;
        assert_eq!(
            state.validate(),
            Err(SimError::InvalidState(
                "consumed combat fairy retains a revival percentage"
            ))
        );
    }

    #[test]
    fn next_card_instance_id_skips_pending_hidden_hand_cards() {
        use crate::content::cards::STRIKE_R_ID;

        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile.clear();
        state.piles.exhaust_pile.clear();
        state.pending_hidden_hand_card_until_end_turn =
            vec![CardInstance::new(CardId::new(15), STRIKE_R_ID)];

        let next = state
            .next_card_instance_id()
            .expect("allocate past pending_hidden");
        assert_eq!(next, 16);
    }
}
