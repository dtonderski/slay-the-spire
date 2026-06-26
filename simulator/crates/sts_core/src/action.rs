use crate::{
    card::CardRarity,
    combat::{
        damage::DamageInfo, DiscardSelectPurpose, DrawSelectPurpose, ExhaustSelectPurpose,
        HandSelectPurpose,
    },
    ids::{CardId, MonsterId},
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
    PlayCard {
        card_id: CardId,
    },
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
    DealDamage {
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
    DealDamageAllAndHealUnblocked {
        source: CardId,
        amount: i32,
    },
    HealPlayer {
        amount: i32,
    },
    GainBlock {
        amount: i32,
    },
    GainTemporaryThorns {
        amount: i32,
    },
    DoublePlayerBlock,
    ApplyVulnerable {
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
    },
    AddRandomColorlessCardToHand {
        rarity: CardRarity,
    },
    MoveCard {
        card_id: CardId,
        from: CardPile,
        to: CardPile,
    },
    RemoveCard {
        card_id: CardId,
        from: CardPile,
    },
    DrawCards {
        count: usize,
    },
    DrawRandomAttacksFromDrawPile {
        count: usize,
    },
    GainEnergy {
        amount: i32,
    },
    LoseHp {
        amount: i32,
        source: HpLossSource,
    },
    SetCannotDraw,
    GainRage {
        amount: i32,
    },
    SetRandomHandCardCostForCombat {
        amount: u8,
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
    GainSadisticNature {
        amount: i32,
    },
    ArmTheBomb {
        turns: i32,
        damage: i32,
    },
    DealUnmodifiedDamage {
        target: crate::MonsterId,
        amount: i32,
    },
    GainMetallicize {
        amount: i32,
    },
    GainStrength {
        amount: i32,
    },
    GainTempStrength {
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
    PlayTopDrawCard {
        target: Option<MonsterId>,
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
    Smith { card_id: CardId },
    RemoveCard { card_id: CardId },
    Lift,
    Dig,
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
