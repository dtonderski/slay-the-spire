use crate::ids::{CardId, MonsterId};
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
        target: MonsterId,
        amount: i32,
    },
    GainBlock {
        amount: i32,
    },
    ApplyVulnerable {
        target: MonsterId,
        amount: i32,
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
