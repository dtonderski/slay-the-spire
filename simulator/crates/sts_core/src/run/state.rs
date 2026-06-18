use crate::{card::CardInstance, combat::CombatState, ids::CardId, ContentId, SimError, SimResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunState {
    pub phase: RunPhase,
    pub deck: Vec<CardInstance>,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub combat: Option<CombatState>,
    pub reward: Option<RewardScreen>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunPhase {
    Combat,
    Reward,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardScreen {
    pub choices: Vec<CardInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunAction {
    SkipReward,
    TakeCardReward { card_id: CardId },
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
            combat: Some(combat),
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
            RunAction::SkipReward => Ok(()),
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
