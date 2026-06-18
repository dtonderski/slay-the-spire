use crate::{
    card::CardInstance,
    combat::CombatState,
    content::character::IRONCLAD_A0_BASE_HP,
    ids::{CardId, ContentId, MonsterId},
    map::{milestone8_fixture, MapRunState},
    potion::{Potion, MAX_POTIONS},
    relic::{apply_start_of_combat_relics, Relic},
    SimError, SimResult,
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
    pub shop: Option<super::shop::ShopScreen>,
    #[serde(default)]
    pub relics: Vec<Relic>,
    #[serde(default)]
    pub potions: Vec<Potion>,
    #[serde(default)]
    pub reward_rng_seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunPhase {
    Combat,
    Reward,
    Rest,
    Shop,
    Idle,
}

pub const REWARD_GOLD_AMOUNT: i32 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewardScreen {
    pub choices: Vec<CardInstance>,
    pub gold_offer: i32,
    pub potion_offer: Option<Potion>,
    pub relic_offer: Option<Relic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunAction {
    SkipReward,
    TakeCardReward { card_id: CardId },
    TakeGoldReward,
    TakePotionReward,
    TakeRelicReward,
    BuyShopCard { slot: usize },
    BuyShopRelic,
    BuyShopPotion,
    UsePotion { slot: usize, target: MonsterId },
    DiscardPotion { slot: usize },
}

impl RunState {
    #[must_use]
    pub fn init_combat(&self, base: CombatState) -> CombatState {
        let mut combat = base;
        combat.player.hp = self.player_hp;
        combat.player.max_hp = self.player_max_hp;
        apply_start_of_combat_relics(&mut combat, &self.relics);
        combat
    }

    #[must_use]
    pub fn combat_fixture() -> Self {
        Self::combat_fixture_with_relics(Vec::new())
    }

    #[must_use]
    pub fn combat_fixture_with_relics(relics: Vec<Relic>) -> Self {
        let deck = crate::content::deck::ironclad_starter_deck();
        let mut run = Self {
            phase: RunPhase::Combat,
            deck,
            player_hp: IRONCLAD_A0_BASE_HP,
            player_max_hp: IRONCLAD_A0_BASE_HP,
            gold: STARTING_GOLD,
            map: None,
            combat: None,
            reward: None,
            shop: None,
            relics,
            potions: Vec::new(),
            reward_rng_seed: 0,
        };
        let combat = run.init_combat(CombatState::initial_fixture());
        run.player_hp = combat.player.hp;
        run.player_max_hp = combat.player.max_hp;
        run.combat = Some(combat);
        run
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
            shop: None,
            relics: Vec::new(),
            potions: Vec::new(),
            reward_rng_seed: 0,
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
            RunAction::TakeGoldReward => {
                if reward.gold_offer > 0 {
                    Ok(())
                } else {
                    Err(SimError::IllegalAction("no gold reward offered"))
                }
            }
            RunAction::TakePotionReward => {
                if reward.potion_offer.is_none() {
                    return Err(SimError::IllegalAction("no potion reward offered"));
                }
                if self.potions.len() >= MAX_POTIONS {
                    return Err(SimError::IllegalAction("potion belt is full"));
                }
                Ok(())
            }
            RunAction::TakeRelicReward => {
                let Some(relic) = reward.relic_offer else {
                    return Err(SimError::IllegalAction("no relic reward offered"));
                };
                if self.relics.contains(&relic) {
                    return Err(SimError::IllegalAction("relic already owned"));
                }
                Ok(())
            }
            RunAction::TakeCardReward { card_id } => {
                if reward.choices.iter().any(|choice| choice.id == card_id) {
                    Ok(())
                } else {
                    Err(SimError::UnknownCard(card_id))
                }
            }
            RunAction::BuyShopCard { .. } | RunAction::BuyShopRelic | RunAction::BuyShopPotion => {
                Err(SimError::IllegalAction("not a reward action"))
            }
            RunAction::UsePotion { .. } | RunAction::DiscardPotion { .. } => {
                Err(SimError::IllegalAction("not a reward action"))
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
