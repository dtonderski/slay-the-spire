use crate::{
    card::CardInstance,
    combat::CombatState,
    content::character::IRONCLAD_A0_BASE_HP,
    ids::CardId,
    map::{milestone8_fixture, MapRunState},
    ContentId, SimError, SimResult,
};
use serde::{Deserialize, Serialize};

pub const STARTING_GOLD: i32 = 99;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub phase: RunPhase,
    pub deck: Vec<CardInstance>,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub gold: i32,
    pub map: Option<MapRunState>,
    pub combat: Option<CombatState>,
    pub reward: Option<RewardScreen>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunPhase {
    Combat,
    Reward,
    Rest,
    Idle,
}

pub const REWARD_GOLD_AMOUNT: i32 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardScreen {
    pub choices: Vec<CardInstance>,
    pub gold_offer: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunAction {
    SkipReward,
    TakeCardReward { card_id: CardId },
    TakeGoldReward,
}

impl RunState {
    #[must_use]
    pub fn combat_fixture() -> Self {
        let deck = crate::content::deck::ironclad_starter_deck();
        let combat = CombatState::initial_fixture();
        Self {
            phase: RunPhase::Combat,
            deck,
            player_hp: combat.player.hp,
            player_max_hp: combat.player.max_hp,
            gold: STARTING_GOLD,
            map: None,
            combat: Some(combat),
            reward: None,
        }
    }

    #[must_use]
    pub fn map_fixture() -> Self {
        Self {
            phase: RunPhase::Idle,
            deck: crate::content::deck::ironclad_starter_deck(),
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            map: Some(milestone8_fixture()),
            combat: None,
            reward: None,
        }
    }

    pub fn next_card_instance_id(&self) -> u64 {
        self.deck
            .iter()
            .map(|card| card.id.get())
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn validate_reward_action(&self, action: RunAction) -> SimResult<()> {
        if self.phase != RunPhase::Reward {
            return Err(SimError::IllegalAction(
                "reward actions require reward phase",
            ));
        }

        let reward = self
            .reward
            .as_ref()
            .ok_or(SimError::InvalidState("reward screen is missing"))?;

        match action {
            RunAction::SkipReward | RunAction::TakeGoldReward => Ok(()),
            RunAction::TakeCardReward { card_id } => {
                if reward.choices.iter().any(|choice| choice.id == card_id) {
                    Ok(())
                } else {
                    Err(SimError::UnknownCard(card_id))
                }
            }
        }
    }

    pub fn count_content_in_deck(&self, content_id: ContentId) -> usize {
        self.deck
            .iter()
            .filter(|card| card.content_id == content_id)
            .count()
    }
}
