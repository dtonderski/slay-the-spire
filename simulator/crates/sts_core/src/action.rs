use crate::{
    combat::damage::DamageInfo,
    ids::{CardId, MonsterId},
};
use serde::{Deserialize, Serialize};

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
    PlayCard {
        card_id: CardId,
    },
    SpendEnergy {
        amount: i32,
    },
    SpendCardEnergy {
        card_id: CardId,
    },
    DealDamage {
        info: DamageInfo,
    },
    DealDamageAll {
        source: CardId,
        amount: i32,
    },
    GainBlock {
        amount: i32,
    },
    ApplyVulnerable {
        target: MonsterId,
        amount: i32,
    },
    AddCardToPile {
        content_id: crate::ContentId,
        to: CardPile,
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
    GainEnergy {
        amount: i32,
    },
    LoseHp {
        amount: i32,
    },
    SetCannotDraw,
    GainFeelNoPain {
        amount: i32,
    },
    GainDarkEmbrace {
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
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestAction {
    Heal,
    OpenSmith,
    Smith { card_id: CardId },
    RemoveCard { card_id: CardId },
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
