use crate::{
    combat::{
        damage::DamageInfo, DiscardSelectPurpose, DrawSelectPurpose, ExhaustSelectPurpose,
        HandSelectPurpose,
    },
    ids::{CardId, ContentId, MonsterId},
    CardInstance,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HpLossSource {
    Card(CardId),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatAction {
    PlayCard {
        card_id: CardId,
        target: Option<MonsterId>,
    },
    EndTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InternalAction {
    ConsumeDuplicationPotion,
    ConsumeDoubleTap,
    ConsumeNecronomicon,
    /// Remove player Vigor after the current Attack card's hits resolve.
    ConsumeVigor,
    PlayCard {
        card_id: CardId,
    },
    /// Time Eater's on-use-card counter deferred until a card-selection screen
    /// closes; the target publishes that lag frame before the card is settled.
    ApplyDeferredTimeWarpCardPlay,
    /// Apply the queued gain after the card's already-queued use effects.
    ApplyTimeWarpStrengthGain,
    PlayCardCopy {
        card_id: CardId,
        /// Retain the source identity after a played Power leaves all piles.
        content_id: ContentId,
    },
    SkipCopiedCardEffectsIfTargetDead {
        target: MonsterId,
    },
    SkipCopiedCardEffectsIfCombatDone,
    /// Resolve monster reactions queued by the original card before a copied card starts.
    ResolvePendingMonsterReactions,
    EndCopiedCardEffects,
    SpendEnergy {
        amount: i32,
    },
    SpendCardEnergy {
        card_id: CardId,
    },
    /// RandomizeHandCostAction queued after Snecko Oil's DrawCardAction.
    RandomizeHandCostsForSneckoOil,
    SetHandCardCostForTurn {
        card_id: CardId,
        cost: u8,
    },
    SetHandCardCostForCombat {
        card_id: CardId,
        cost: u8,
    },
    /// `AbstractCard.modifyCostForCombat`: reduce the combat-long cost while
    /// preserving any distinct current-turn override.
    // Preserve the existing snapshot/trace wire name; the action searches all piles.
    #[serde(rename = "ReduceHandCardCostForCombat")]
    ReduceCardCostForCombat {
        card_id: CardId,
        amount: u8,
    },
    DealDamage {
        info: DamageInfo,
    },
    /// Calculate a card's final attack amount before its following queued use
    /// effects, then resolve that prepared amount later.
    PrepareCardDamage {
        info: DamageInfo,
    },
    /// Resolution of a prepared card amount still applies block, thorns, and
    /// monster reactions without recalculating attacker stat modifiers.
    DealPreparedDamage {
        info: DamageInfo,
    },
    /// BaneAction checks for Poison only when its separately queued second hit resolves.
    DealBaneDamageIfPoisoned {
        info: DamageInfo,
    },
    DealBodySlamDamage {
        source: CardId,
        target: MonsterId,
    },
    DealHandOfGreedDamage {
        info: DamageInfo,
        gold: i32,
    },
    DealRitualDaggerDamage {
        info: DamageInfo,
        growth: i32,
    },
    DealDamageAndHealUnblocked {
        info: DamageInfo,
    },
    DealDamageRandomEnemy {
        source: CardId,
        amount: i32,
    },
    DealFeedDamage {
        info: DamageInfo,
        max_hp_gain: i32,
    },
    DealDamageAll {
        source: CardId,
        amount: i32,
    },
    /// Fire Breathing's queued `DamageAllEnemiesAction` from a draw callback.
    /// The amount is captured when the source power receives the callback.
    FireBreathingDamage {
        amount: i32,
    },
    DealDamageAllRepeated {
        source: CardId,
        amount: i32,
        times: i32,
    },
    DealDamageAllAndHealUnblocked {
        source: CardId,
        amount: i32,
    },
    DealDamageAndGainBlockUnblocked {
        info: crate::combat::DamageInfo,
    },
    /// Guardian Sharp Hide (`onUseCard`) damage. It is queued by card-use
    /// powers before UseCardAction settles the source card.
    DealSharpHideDamageToPlayer {
        amount: i32,
    },
    /// Other thorns-style damage to the player, including Beat of Death.
    DealThornsDamageToPlayer {
        amount: i32,
    },
    HealPlayer {
        amount: i32,
    },
    GainBlock {
        amount: i32,
    },
    /// A card precomputed its final block through applyPowers before queuing one GainBlockAction.
    GainPrecomputedCardBlock {
        amount: i32,
    },
    GainBlockDirect {
        amount: i32,
    },
    /// BlockPotion's GainBlockAction: unmodified block with potion error semantics.
    GainBlockFromPotion {
        amount: i32,
    },
    /// Feel No Pain's on-exhaust block is queued after the exhaust action and
    /// is not prevented by Panic Button's NoBlockPower.
    GainBlockFromExhaust {
        amount: i32,
    },
    GainMonsterBlock {
        target: MonsterId,
        amount: i32,
    },
    /// Compulsive (`ReactivePower`) queues `RollMoveAction` with addToBot.
    RerollWrithingMassAfterAttack {
        target: MonsterId,
    },
    PreventBlockGain {
        turns: i32,
    },
    GainTemporaryThorns {
        amount: i32,
    },
    DoublePlayerBlock,
    ApplyVulnerable {
        target: MonsterId,
        amount: i32,
    },
    ApplyMark {
        target: MonsterId,
        amount: i32,
    },
    ApplyPlayerVulnerable {
        amount: i32,
    },
    ReduceMonsterStrength {
        target: MonsterId,
        amount: i32,
    },
    ReduceMonsterStrengthThisTurn {
        target: MonsterId,
        amount: i32,
    },
    AddCardToPile {
        content_id: crate::ContentId,
        to: CardPile,
    },
    AddGeneratedCardToPile {
        content_id: crate::ContentId,
        to: CardPile,
        temp_cost: Option<u8>,
        temp_cost_turn_only: bool,
    },
    AddGeneratedUpgradedCardToPile {
        content_id: crate::ContentId,
        to: CardPile,
    },
    AddGeneratedCardsToHandWhileSourceInLimbo {
        content_id: crate::ContentId,
        source_card_id: CardId,
        count: usize,
        temp_cost: Option<u8>,
        temp_cost_turn_only: bool,
    },
    AddGeneratedHandCardBeforePendingDraw {
        content_id: crate::ContentId,
        temp_cost: Option<u8>,
        temp_cost_turn_only: bool,
    },
    AddStatEquivalentCopyToPile {
        card: CardInstance,
        to: CardPile,
    },
    AddCardInstanceToHandOrDiscard {
        card: CardInstance,
    },
    /// Transfer a physical Stasis card at the queued return boundary. Until
    /// then the monster retains ownership, including its reserved instance ID.
    ReturnMonsterStasisCard {
        monster_id: MonsterId,
    },
    AddGeneratedCardToDrawPileRandomSpot {
        content_id: crate::ContentId,
    },
    AddGeneratedCardToDrawPileRandomSpotWithCost {
        content_id: crate::ContentId,
        temp_cost: Option<u8>,
        temp_cost_turn_only: bool,
    },
    AddRandomColorlessCardToHand {
        temp_cost: Option<u8>,
        upgrade: bool,
    },
    /// Transmutation: MakeTempCardInHand sees the played card in limbo (removed
    /// from hand), so hand capacity is computed without the X-cost source
    /// (FIDL00413: X=16 must fill to 10, not stall at 9).
    AddRandomColorlessCardsToHandWhileSourceInLimbo {
        source_card_id: CardId,
        count: usize,
        temp_cost: Option<u8>,
        upgrade: bool,
    },
    MoveCard {
        card_id: CardId,
        from: CardPile,
        to: CardPile,
    },
    /// Move one hand card to discard and run the target's manual-discard
    /// counter and card callbacks.
    /// UnloadAction.update snapshots non-attacks from the live hand, then
    /// queues their individual discards on top in reverse hand order.
    DiscardNonAttackHandCards,
    ManualDiscardCard {
        card_id: CardId,
    },
    ReturnExhaustCardToHand {
        card_id: CardId,
    },
    ForethoughtAutoMove {
        source_card_id: CardId,
        card_id: CardId,
    },
    ExhaustRandomHandCardExcept {
        excluded_card_id: CardId,
    },
    /// `ExhaustAllNonAttackAction`: one hand snapshot of non-attacks.
    /// Soulbound replacements are not in that snapshot; a Necronomicon copy's
    /// second use() exhausts them (FIDL01518 Feel No Pain 9).
    ExhaustAllNonAttackCards {
        excluded_card_id: CardId,
    },
    /// Storm of Steel resolves against the hand present when BladeFuryAction
    /// updates, so copied plays re-read the hand after the original made Shivs.
    ResolveStormOfSteel {
        source_card_id: CardId,
        upgraded: bool,
    },
    /// Exhaust every other hand card, then deal `amount` once per exhausted card.
    /// Hit count is decided at resolve time so Double Tap / Necronomicon copies
    /// with an empty hand deal zero hits (FIDL00237 Fiend Fire + Double Tap).
    ResolveFiendFire {
        source_card_id: CardId,
        target: MonsterId,
        amount: i32,
    },
    RemoveCard {
        card_id: CardId,
        from: CardPile,
    },
    /// Potion addToBot selectors read the live hand only when this action starts.
    OpenElixirSelection,
    OpenGamblersBrewSelection,
    /// Discovery potions generate their offer only when their queued action starts.
    OpenPotionCardReward {
        reward_kind: crate::combat::PotionCardRewardKind,
    },
    DrawCards {
        count: usize,
    },
    DrawCardsWithoutEvolve {
        count: usize,
    },
    DrawCardsWhilePlayedCardIsInLimbo {
        card_id: CardId,
        count: usize,
    },
    DrawCardsWhilePlayedCardIsInLimboWithoutEvolve {
        card_id: CardId,
        count: usize,
    },
    DrawCardsFromInkBottle {
        count: usize,
    },
    ShuffleDiscardIntoDraw,
    DeepBreathShuffleDiscardIntoDraw,
    DrawCardsIfNoAttacksInHand {
        count: usize,
    },
    /// Draw `count` cards, then discard those whose costForTurn is not 0
    /// (`ScrapeFollowUpAction` over `DrawCardAction.drawnCards`).
    DrawThenScrapeDiscard {
        count: usize,
    },
    DrawRandomAttacksFromDrawPile {
        count: usize,
    },
    GainEnergy {
        amount: i32,
    },
    /// GainEnergyAction queued by Energy Potion, retaining its overflow error.
    GainEnergyFromPotion {
        amount: i32,
    },
    /// VoidCard.triggerWhenDrawn addToBot's LoseEnergyAction.
    LoseEnergy {
        amount: i32,
    },
    LoseHp {
        amount: i32,
        source: HpLossSource,
    },
    SetCannotDraw,
    /// Orange Pellets' RemoveDebuffsAction, queued after the played card's own effects.
    ClearPlayerDebuffs,
    GainRage {
        amount: i32,
    },
    SetRandomHandCardCostForCombat {
        amount: u8,
        excluded_card_id: CardId,
    },
    UpgradeHandCardsExcept {
        card_id: CardId,
    },
    UpgradeHandCard {
        card_id: CardId,
    },
    IncreaseRampageDamage {
        card_id: CardId,
        amount: i32,
    },
    ResolveSteamBarrier {
        card_id: CardId,
    },
    ResolveFollowUpEnergy {
        should_gain: bool,
    },
    GainFeelNoPain {
        amount: i32,
    },
    GainDarkEmbrace {
        amount: i32,
    },
    GainBarricade {
        amount: i32,
    },
    GainEvolve {
        amount: i32,
    },
    GainBerserk {
        amount: i32,
    },
    GainFasting {
        amount: i32,
    },
    GainLikeWater {
        amount: i32,
    },
    GainNirvana {
        amount: i32,
    },
    GainRupture {
        amount: i32,
    },
    GainJuggernaut {
        amount: i32,
    },
    GainBrutality {
        amount: i32,
    },
    GainMayhem {
        amount: i32,
    },
    GainPanache {
        amount: i32,
    },
    GainCombust {
        amount: i32,
    },
    GainDoubleTap {
        amount: i32,
    },
    GainFireBreathing {
        amount: i32,
    },
    GainCorruption {
        amount: i32,
    },
    /// Enter Watcher Divinity stance (triple attack damage).
    EnterDivinity,
    /// Blasphemy EndTurnDeathPower — die at end of this turn.
    ApplyEndTurnDeath,
    GainSadisticNature {
        amount: i32,
    },
    GainMagnetism {
        amount: i32,
    },
    GainCreativeAI {
        amount: i32,
    },
    GainStorm {
        amount: i32,
    },
    GainAfterImage {
        amount: i32,
    },
    GainStaticDischarge {
        amount: i32,
    },
    GainThorns {
        amount: i32,
    },
    IncreaseMaxOrbs {
        amount: i32,
    },
    /// Recursion / RedoAction: evoke the rightmost orb and channel it again.
    RecurseRightmostOrb,
    /// StormPower.onUseCard addToBot ChannelAction(Lightning).
    ChannelLightning,
    /// Coolheaded.use addToBot ChannelAction(Frost).
    ChannelFrost,
    /// Darkness.use addToBot ChannelAction(Dark).
    ChannelDark,
    /// Darkness+ addToBot DarkImpulseAction: each Dark orb onEndOfTurn.
    DarkImpulse,
    /// PressEndTurnButtonAction requests an end turn without settling it inline.
    ForceEndTurn,
    /// Settle the requested end turn after UseCardAction moves its source card.
    SettleForcedEndTurn,
    /// GremlinHorn.onMonsterDeath addToBots GainEnergy + Draw at the death.
    ApplyGremlinHornOnDeath,
    /// JudgementAction: if target HP <= threshold, set HP to 0.
    ExecuteJudgement {
        target: MonsterId,
        threshold: i32,
    },
    /// Lightning.onEndOfTurn / LightningOrbPassiveAction.
    LightningOrbPassive,
    ArmTheBomb {
        turns: i32,
        damage: i32,
    },
    DealUnmodifiedDamage {
        target: crate::MonsterId,
        amount: i32,
    },
    DealUnmodifiedDamageRandom {
        amount: i32,
    },
    GainMetallicize {
        amount: i32,
    },
    GainStrength {
        amount: i32,
    },
    /// LimitBreakAction reads the live Strength power at execution time.
    DoublePlayerStrength,
    GainMantra {
        amount: i32,
    },
    EnterCalm,
    EnterWrath,
    ExitCalm,
    /// FlurryOfBlows.triggerExhaustedCardsOnStanceChange addToBots DiscardToHandAction.
    DiscardToHand {
        card_id: CardId,
    },
    GainDexterity {
        amount: i32,
    },
    /// SpeedPotion.use's ordered addToBot ApplyPowerAction pair.
    GainDexterityFromSpeedPotion {
        amount: i32,
    },
    ApplyDexLossFromSpeedPotion {
        amount: i32,
    },
    GainTempStrength {
        amount: i32,
    },
    GainIntangible {
        amount: i32,
    },
    GainRitual {
        amount: i32,
    },
    GainArtifact {
        amount: i32,
    },
    /// AncientPotion.use addToBot ApplyPowerAction; preserves potion errors.
    GainArtifactFromPotion {
        amount: i32,
    },
    UpgradeCombatCards,
    ApplyWeak {
        target: MonsterId,
        amount: i32,
    },
    ApplyWeakIfTargetAttacking {
        target: MonsterId,
        amount: i32,
    },
    ApplyPoison {
        target: MonsterId,
        amount: i32,
    },
    TriggerMarks,
    LoseMonsterHp {
        target: MonsterId,
        amount: i32,
    },
    CardExhausted {
        card_id: CardId,
    },
    /// Unceasing Top draws after the exhaust callback queue has settled.
    UnceasingTopDraw,
    HandCardExhausted {
        card_id: CardId,
    },
    PlayTopDrawCard {
        target: Option<MonsterId>,
        exhaust_played_card: bool,
        random_living_target: bool,
    },
    /// Resolve a card selected by PlayTopCardAction after the current action
    /// queue drains. Vanilla keeps the selected card in limbo while the parent
    /// UseCardAction settles, then processes the card queue.
    ResolveTopDrawCard {
        card_id: CardId,
        target: Option<MonsterId>,
        exhaust_played_card: bool,
    },
    /// Finish one card-queue item after all actions produced by that card have run.
    EndPlayTopCardResolution {
        card_id: CardId,
        deferred_destination: Option<CardPile>,
        previous_card_in_use: Option<CardId>,
        previous_force_exhaust: bool,
    },
    PutHandCardOnTopOfDraw {
        card_id: CardId,
    },
    CopyHandCardToHand {
        card_id: CardId,
    },
    AwaitHandSelect {
        source_card_id: CardId,
        purpose: HandSelectPurpose,
    },
    AwaitDrawSelect {
        source_card_id: CardId,
        purpose: DrawSelectPurpose,
    },
    AwaitDiscardSelect {
        source_card_id: CardId,
        purpose: DiscardSelectPurpose,
    },
    AwaitCopiedDiscardSelect {
        purpose: DiscardSelectPurpose,
    },
    /// Copy-owned PutOnDeck / Forethought select. Does not settle the original source.
    AwaitCopiedHandSelect {
        purpose: HandSelectPurpose,
    },
    AwaitExhaustSelect {
        source_card_id: CardId,
        purpose: ExhaustSelectPurpose,
    },
    OpenDiscoveryCardReward {
        source_card_id: CardId,
    },
    /// Mechanics introduced by Silent/Defect/Watcher cards (Prismatic Shard).
    Foreign(ForeignAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrbKind {
    Lightning,
    Frost,
    Dark,
    Plasma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ForeignMonsterPower {
    Choke,
    CorpseExplosion,
    BlockReturn,
    LockOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StanceKind {
    Neutral,
    Calm,
    Wrath,
    Divinity,
}

/// Where `MakeTempCard*` actions put a created card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TempCardDestination {
    Hand,
    DiscardPile,
    /// `MakeTempCardInDrawPileAction(randomSpot = true)`.
    DrawPileRandom,
}

/// Source actions used by the cross-color cards. Each variant mirrors one
/// vanilla action (named in its documentation) and computes state-dependent
/// amounts when it resolves, so Burst/Echo/Double Tap copies recompute them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForeignAction {
    /// A purgeOnUse card-queue copy runs use() again against live state. Keep
    /// its stat-equivalent source snapshot, never relocate the original card.
    CopiedUse {
        card: CardInstance,
        target: Option<MonsterId>,
    },
    /// `AbstractPlayer.useCard` removes the played card from hand before its
    /// actions resolve. A copy whose source already left hand is a no-op.
    StageInLimbo {
        card_id: CardId,
    },
    /// `ChannelAction`.
    Channel {
        orb: OrbKind,
    },
    /// `ChannelAction(AbstractOrb.getRandomOrb(true))` (Chaos).
    ChannelRandom,
    /// `EvokeOrbAction(1)` / `EvokeWithoutRemovingOrbAction(1)`.
    EvokeFirst {
        remove: bool,
    },
    /// `EvokeAllOrbsAction`.
    EvokeAll,
    /// `RemoveAllOrbsAction`.
    RemoveAllOrbs,
    /// `DecreaseMaxOrbAction`.
    DecreaseMaxOrbs {
        amount: i32,
    },
    /// LoopPower: first orb's `onStartOfTurn` then `onEndOfTurn`.
    TriggerFirstOrb,
    /// `BarrageAction`: one hit per filled orb slot.
    Barrage {
        info: DamageInfo,
    },
    /// `GashAction` (Claw).
    Gash {
        card_id: CardId,
        amount: i32,
    },
    /// `HeadStompAction` (Sash Whip).
    WeakIfPreviousAttack {
        target: MonsterId,
        amount: i32,
    },
    /// `SanctityAction`.
    DrawIfPreviousSkill {
        count: i32,
    },
    /// `AggregateEnergyAction`.
    AggregateEnergy {
        divide: i32,
    },
    /// Auto Shields' use-time `currentBlock == 0` check.
    BlockIfNoBlock {
        amount: i32,
    },
    /// `BouncingFlaskAction`; `None` rolls the initial target.
    BouncingFlask {
        target: Option<MonsterId>,
        amount: i32,
        times: i32,
    },
    /// `CalculatedGambleAction`.
    CalculatedGamble {
        upgraded: bool,
    },
    /// `DiscardAction(amount, isRandom)`.
    DiscardFromHand {
        amount: i32,
        random: bool,
    },
    /// `DoublePoisonAction` / `TriplePoisonAction`: add `multiplier` x current Poison.
    MultiplyPoison {
        target: MonsterId,
        multiplier: i32,
    },
    /// `ApplyPowerAction(FocusPower)`.
    GainFocus {
        amount: i32,
    },
    /// `ApplyPowerAction(VigorPower/BufferPower)`.
    GainVigor {
        amount: i32,
    },
    GainBuffer {
        amount: i32,
    },
    /// `DoubleEnergyAction`.
    DoubleEnergy,
    /// `DrawCardAction(1, EscapePlanAction)`.
    EscapePlan {
        block: i32,
    },
    /// `ExpertiseAction` (Expertise, Scrawl).
    Expertise {
        hand_size: i32,
    },
    /// `FTLAction`.
    Ftl {
        info: DamageInfo,
        threshold: i32,
    },
    /// `DamagePerAttackPlayedAction` (Finisher).
    Finisher {
        info: DamageInfo,
    },
    /// `FlechetteAction`.
    Flechettes {
        info: DamageInfo,
    },
    /// `ForeignInfluenceAction`.
    ForeignInfluence {
        upgraded: bool,
    },
    /// `IncreaseMiscAction` (Genetic Algorithm).
    IncreaseMisc {
        card_id: CardId,
        amount: i32,
    },
    /// `HeelHookAction`.
    HeelHook {
        info: DamageInfo,
    },
    /// `IndignationAction`.
    Indignation {
        amount: i32,
    },
    /// `InnerPeaceAction`.
    InnerPeace {
        amount: i32,
    },
    /// `MeditateAction`.
    Meditate {
        amount: i32,
    },
    /// `RemoveAllBlockAction` (Melter).
    RemoveMonsterBlock {
        target: MonsterId,
    },
    /// `SetupAction`.
    Setup,
    /// `SunderAction`.
    Sunder {
        info: DamageInfo,
        energy: i32,
    },
    /// `AllCostToHandAction(0)` (All for One).
    AllCostToHand {
        cost: i32,
    },
    /// `ApplyBulletTimeAction`.
    BulletTime,
    /// `ConjureBladeAction`.
    ConjureBlade {
        x: i32,
    },
    /// `FissionAction`.
    Fission {
        upgraded: bool,
    },
    /// `ModifyDamageAction` (Glass Knife).
    ModifyDamage {
        card_id: CardId,
        amount: i32,
    },
    /// `MulticastAction` after X was fixed at use time.
    Multicast {
        effect: i32,
    },
    /// `NightmareAction`.
    Nightmare {
        amount: i32,
    },
    /// `OmniscienceAction`.
    Omniscience {
        plays: i32,
    },
    /// Thunder Strike: one `NewThunderStrikeAction` per Lightning channeled.
    ThunderStrike {
        source: CardId,
        amount: i32,
    },
    /// `ShuffleAllAction` + `ShuffleAction` (Reboot).
    Reboot,
    /// `BetterDrawPileToHandAction` (Seek).
    Seek {
        amount: i32,
    },
    /// `SkipEnemiesTurnAction`.
    SkipEnemiesTurn,
    /// Wish's `ChooseOneAction`.
    Wish {
        upgraded: bool,
    },
    /// `ScryAction`.
    Scry {
        amount: i32,
    },
    /// `MakeTempCardInHandAction(returnTrulyRandomCardInCombat(type))` with an optional cost for turn.
    RandomCombatCardToHand {
        card_type: crate::card::CardType,
        cost_for_turn: Option<u8>,
    },
    /// HelloPower: `MakeTempCardInHandAction(getCard(COMMON, cardRandomRng))`.
    RandomCommonCardToHand,
    /// `ApplyPowerAction` for a foreign player power.
    GainPower {
        power: crate::combat::ForeignPower,
        amount: i32,
    },
    /// `ApplyPowerAction` for a foreign monster debuff.
    ApplyMonsterPower {
        target: MonsterId,
        power: ForeignMonsterPower,
        amount: i32,
    },
    /// `ChangeStanceAction`.
    ChangeStance {
        stance: StanceKind,
    },
    /// `MakeTempCardIn*Action` for a special card.
    MakeTempCard {
        content_id: ContentId,
        destination: TempCardDestination,
        count: i32,
        upgraded: bool,
    },
    /// `GainGoldAction` (Fame and Fortune).
    GainGold {
        amount: i32,
    },
    /// `ApplyPowerAction(PlatedArmorPower)` (Live Forever).
    GainPlatedArmor {
        amount: i32,
    },
    /// `ObtainPotionAction(returnRandomPotion(true))` (Alchemize).
    ObtainRandomPotion,
    /// `ApplyPowerAction(VulnerablePower)` against every monster.
    ApplyVulnerableAll {
        amount: i32,
    },
    /// `ApplyPowerAction(WeakPower)` against every monster (Wave of the Hand).
    ApplyWeakAll {
        amount: i32,
    },
    /// `DamageAllEnemiesAction(createDamageMatrix(amount, true), THORNS)`.
    DamageAllThorns {
        amount: i32,
    },
    /// Spirit Shield: `applyPowers` block from the other hand cards.
    SpiritShield {
        per_card: i32,
    },
    /// Stack: `applyPowers` block from the discard pile size.
    Stack {
        bonus: i32,
    },
    /// EndlessAgony/DeusExMachina `triggerWhenDrawn` follow-ups.
    EndlessAgonyCopy {
        card_id: CardId,
    },
    DeusExMachinaDrawn {
        card_id: CardId,
        miracles: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestAction {
    Heal,
    OpenSmith,
    OpenRemove,
    Smith { card_id: CardId },
    RemoveCard { card_id: CardId },
    Lift,
    Dig,
    Recall,
    Proceed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventAction {
    Choose { choice_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardPile {
    Hand,
    DrawPile,
    DiscardPile,
    ExhaustPile,
}
