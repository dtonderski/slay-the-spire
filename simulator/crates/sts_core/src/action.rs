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
    PlayCard {
        card_id: CardId,
    },
    SpendEnergy {
        amount: i32,
    },
    DealDamage {
        info: DamageInfo,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardPile {
    Hand,
    DrawPile,
    DiscardPile,
    ExhaustPile,
}
