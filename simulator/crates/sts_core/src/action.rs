use crate::{
    combat::{
        damage::DamageInfo, DiscardSelectPurpose, DrawSelectPurpose, ExhaustSelectPurpose,
        HandSelectPurpose,
    },
    ids::{CardId, MonsterId},
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
    PlayCardCopy {
        card_id: CardId,
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
    SetHandCardCostForTurn {
        card_id: CardId,
        cost: u8,
    },
    SetHandCardCostForCombat {
        card_id: CardId,
        cost: u8,
    },
    DealDamage {
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
    /// Guardian Sharp Hide (`onUseCard`) thorns damage to the player.
    /// Queued after `card.use()` effects, like STS `addToBot(DamageAction)`.
    DealThornsDamageToPlayer {
        amount: i32,
    },
    HealPlayer {
        amount: i32,
    },
    GainBlock {
        amount: i32,
    },
    GainBlockDirect {
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
    /// Conclude.use addToBot EndTurnAction.
    ForceEndTurn,
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
    GainMantra {
        amount: i32,
    },
    EnterCalm,
    EnterWrath,
    ExitCalm,
    GainDexterity {
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
    UpgradeCombatCards,
    ApplyWeak {
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
    AwaitExhaustSelect {
        source_card_id: CardId,
        purpose: ExhaustSelectPurpose,
    },
    OpenDiscoveryCardReward {
        source_card_id: CardId,
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
