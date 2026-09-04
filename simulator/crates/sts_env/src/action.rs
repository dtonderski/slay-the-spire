use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};
use sts_core::adapter_internals::{
    CombatAction, EventAction, MapAction, RestAction, RunAction, RunDecisionAction, RunState,
};

/// A monotonic, environment-owned token for one decision boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DecisionRevision(u64);

impl DecisionRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// A complete player choice expressed only with decision-local visible slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublicChoice {
    PlayHandSlot {
        hand_slot: u16,
        target_slot: Option<u16>,
    },
    EndTurn,
    ChooseEventOption {
        option_slot: u16,
    },
    ToggleGridCard {
        card_slot: u16,
    },
    ConfirmGrid,
    CancelGrid,
    ChooseMapNode {
        node_slot: u16,
    },
    RestHeal,
    RestOpenSmith,
    RestOpenRemove,
    RestSmith {
        card_slot: u16,
    },
    RestRemoveCard {
        card_slot: u16,
    },
    RestLift,
    RestDig,
    RestRecall,
    RestProceed,
    SkipReward,
    CloseCardReward,
    TakeCardReward {
        reward_slot: u16,
    },
    TakeSingingBowlReward,
    TakeGoldReward,
    TakeStolenGoldReward,
    TakePotionReward {
        reward_slot: u16,
    },
    TakeRelicReward,
    TakeRelicRewardAt {
        reward_slot: u16,
    },
    TakeSapphireKey,
    TakeEmeraldKey,
    ChooseBossRelicReward {
        reward_slot: u16,
    },
    Proceed,
    OpenChest,
    OpenCardReward,
    OpenQueuedCardReward {
        reward_slot: u16,
    },
    SkipPotionReward,
    BuyShopCard {
        shop_slot: u16,
    },
    BuyShopRelic {
        shop_slot: u16,
    },
    BuyShopPotion {
        shop_slot: u16,
    },
    UsePotionSlot {
        potion_slot: u16,
        target_slot: Option<u16>,
    },
    DiscardPotionSlot {
        potion_slot: u16,
    },
    ToggleVisibleCard {
        option_slot: u16,
    },
    ChooseVisibleOption {
        option_slot: u16,
    },
    ConfirmSelection,
    ConfirmSelectionWithoutRetrieval,
    SkipSelection,
    EnterShop,
    LeaveShop,
    OpenShopRemove,
}

impl PublicChoice {
    #[must_use]
    pub const fn family(self) -> &'static str {
        match self {
            Self::PlayHandSlot { .. } | Self::EndTurn => "combat",
            Self::ChooseEventOption { .. } => "event",
            Self::ToggleGridCard { .. } | Self::ConfirmGrid | Self::CancelGrid => "grid",
            Self::ChooseMapNode { .. } => "map",
            Self::RestHeal
            | Self::RestOpenSmith
            | Self::RestOpenRemove
            | Self::RestSmith { .. }
            | Self::RestRemoveCard { .. }
            | Self::RestLift
            | Self::RestDig
            | Self::RestRecall
            | Self::RestProceed => "rest",
            _ => "run",
        }
    }

    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::PlayHandSlot { .. } => "play_hand_slot",
            Self::EndTurn => "end_turn",
            Self::ChooseEventOption { .. } => "choose_event_option",
            Self::ToggleGridCard { .. } => "toggle_grid_card",
            Self::ConfirmGrid => "confirm_grid",
            Self::CancelGrid => "cancel_grid",
            Self::ChooseMapNode { .. } => "choose_map_node",
            Self::RestHeal => "rest_heal",
            Self::RestOpenSmith => "rest_open_smith",
            Self::RestOpenRemove => "rest_open_remove",
            Self::RestSmith { .. } => "rest_smith",
            Self::RestRemoveCard { .. } => "rest_remove_card",
            Self::RestLift => "rest_lift",
            Self::RestDig => "rest_dig",
            Self::RestRecall => "rest_recall",
            Self::RestProceed => "rest_proceed",
            Self::SkipReward => "skip_reward",
            Self::CloseCardReward => "close_card_reward",
            Self::TakeCardReward { .. } => "take_card_reward",
            Self::TakeSingingBowlReward => "take_singing_bowl_reward",
            Self::TakeGoldReward => "take_gold_reward",
            Self::TakeStolenGoldReward => "take_stolen_gold_reward",
            Self::TakePotionReward { .. } => "take_potion_reward",
            Self::TakeRelicReward => "take_relic_reward",
            Self::TakeRelicRewardAt { .. } => "take_relic_reward_at",
            Self::TakeSapphireKey => "take_sapphire_key",
            Self::TakeEmeraldKey => "take_emerald_key",
            Self::ChooseBossRelicReward { .. } => "choose_boss_relic_reward",
            Self::Proceed => "proceed",
            Self::OpenChest => "open_chest",
            Self::OpenCardReward => "open_card_reward",
            Self::OpenQueuedCardReward { .. } => "open_queued_card_reward",
            Self::SkipPotionReward => "skip_potion_reward",
            Self::BuyShopCard { .. } => "buy_shop_card",
            Self::BuyShopRelic { .. } => "buy_shop_relic",
            Self::BuyShopPotion { .. } => "buy_shop_potion",
            Self::UsePotionSlot { .. } => "use_potion_slot",
            Self::DiscardPotionSlot { .. } => "discard_potion_slot",
            Self::ToggleVisibleCard { .. } => "toggle_visible_card",
            Self::ChooseVisibleOption { .. } => "choose_visible_option",
            Self::ConfirmSelection => "confirm_selection",
            Self::ConfirmSelectionWithoutRetrieval => "confirm_selection_without_retrieval",
            Self::SkipSelection => "skip_selection",
            Self::EnterShop => "enter_shop",
            Self::LeaveShop => "leave_shop",
            Self::OpenShopRemove => "open_shop_remove",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicChoiceRequest {
    pub revision: DecisionRevision,
    pub choice: PublicChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairError {
    DecisionUnavailable,
    StaleDecision,
    InvalidChoice,
    RevisionExhausted,
    InvalidSeed,
}

impl fmt::Display for FairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DecisionUnavailable => "decision unavailable",
            Self::StaleDecision => "decision is stale",
            Self::InvalidChoice => "choice is invalid",
            Self::RevisionExhausted => "decision revision exhausted",
            Self::InvalidSeed => "seed is invalid",
        })
    }
}

impl std::error::Error for FairError {}

pub(crate) fn projected_choices(
    run: &RunState,
) -> Result<Vec<(PublicChoice, RunDecisionAction)>, FairError> {
    let actions = sts_core::adapter_internals::legal_run_decision_actions(run)
        .map_err(|_| FairError::DecisionUnavailable)?;
    let mut seen = BTreeSet::new();
    let mut projected = Vec::with_capacity(actions.len());
    for action in actions {
        let choice = project_action(run, action)?;
        if !seen.insert(choice) {
            return Err(FairError::DecisionUnavailable);
        }
        projected.push((choice, action));
    }
    Ok(projected)
}

fn project_action(run: &RunState, action: RunDecisionAction) -> Result<PublicChoice, FairError> {
    Ok(match action {
        RunDecisionAction::Combat(CombatAction::PlayCard { card_id, target }) => {
            let combat = run.combat.as_ref().ok_or(FairError::DecisionUnavailable)?;
            let hand_slot = combat
                .piles
                .hand
                .iter()
                .position(|card| card.id == card_id)
                .ok_or(FairError::DecisionUnavailable)
                .and_then(public_slot)?;
            let target_slot = target
                .map(|id| {
                    combat
                        .monsters
                        .iter()
                        .position(|monster| monster.id == id)
                        .ok_or(FairError::DecisionUnavailable)
                        .and_then(public_slot)
                })
                .transpose()?;
            PublicChoice::PlayHandSlot {
                hand_slot,
                target_slot,
            }
        }
        RunDecisionAction::Combat(CombatAction::EndTurn) => PublicChoice::EndTurn,
        RunDecisionAction::Event(EventAction::Choose { choice_index }) => {
            PublicChoice::ChooseEventOption {
                option_slot: public_slot(choice_index)?,
            }
        }
        RunDecisionAction::GridSelect { index } => PublicChoice::ToggleGridCard {
            card_slot: public_slot(index)?,
        },
        RunDecisionAction::GridConfirm => PublicChoice::ConfirmGrid,
        RunDecisionAction::GridCancel => PublicChoice::CancelGrid,
        RunDecisionAction::Map(MapAction::ChooseNode { node_id }) => {
            let slot = run
                .map
                .as_ref()
                .and_then(|map| map.map.nodes.iter().position(|node| node.id == node_id))
                .ok_or(FairError::DecisionUnavailable)?;
            PublicChoice::ChooseMapNode {
                node_slot: public_slot(slot)?,
            }
        }
        RunDecisionAction::Rest(RestAction::Heal) => PublicChoice::RestHeal,
        RunDecisionAction::Rest(RestAction::OpenSmith) => PublicChoice::RestOpenSmith,
        RunDecisionAction::Rest(RestAction::OpenRemove) => PublicChoice::RestOpenRemove,
        RunDecisionAction::Rest(RestAction::Smith { card_id }) => PublicChoice::RestSmith {
            card_slot: deck_slot(run, card_id)?,
        },
        RunDecisionAction::Rest(RestAction::RemoveCard { card_id }) => {
            PublicChoice::RestRemoveCard {
                card_slot: deck_slot(run, card_id)?,
            }
        }
        RunDecisionAction::Rest(RestAction::Lift) => PublicChoice::RestLift,
        RunDecisionAction::Rest(RestAction::Dig) => PublicChoice::RestDig,
        RunDecisionAction::Rest(RestAction::Recall) => PublicChoice::RestRecall,
        RunDecisionAction::Rest(RestAction::Proceed) => PublicChoice::RestProceed,
        RunDecisionAction::Run(action) => project_run_action(run, action)?,
    })
}

fn project_run_action(run: &RunState, action: RunAction) -> Result<PublicChoice, FairError> {
    Ok(match action {
        RunAction::SkipReward => PublicChoice::SkipReward,
        RunAction::CloseCardReward => PublicChoice::CloseCardReward,
        RunAction::TakeCardReward { card_id } => {
            let slot = run
                .reward
                .as_ref()
                .and_then(|reward| reward.choices.iter().position(|card| card.id == card_id))
                .ok_or(FairError::DecisionUnavailable)?;
            PublicChoice::TakeCardReward {
                reward_slot: public_slot(slot)?,
            }
        }
        RunAction::TakeSingingBowlReward => PublicChoice::TakeSingingBowlReward,
        RunAction::TakeGoldReward => PublicChoice::TakeGoldReward,
        RunAction::TakeStolenGoldReward => PublicChoice::TakeStolenGoldReward,
        RunAction::TakePotionReward { index } => PublicChoice::TakePotionReward {
            reward_slot: public_slot(index)?,
        },
        RunAction::TakeRelicReward => PublicChoice::TakeRelicReward,
        RunAction::TakeRelicRewardAt { index } => PublicChoice::TakeRelicRewardAt {
            reward_slot: public_slot(index)?,
        },
        RunAction::TakeSapphireKey => PublicChoice::TakeSapphireKey,
        RunAction::TakeEmeraldKey => PublicChoice::TakeEmeraldKey,
        RunAction::ChooseBossRelicReward { index } => PublicChoice::ChooseBossRelicReward {
            reward_slot: public_slot(index)?,
        },
        RunAction::Proceed => PublicChoice::Proceed,
        RunAction::OpenChest => PublicChoice::OpenChest,
        RunAction::OpenCardReward => PublicChoice::OpenCardReward,
        RunAction::OpenQueuedCardReward { index } => PublicChoice::OpenQueuedCardReward {
            reward_slot: public_slot(index)?,
        },
        RunAction::SkipPotionReward => PublicChoice::SkipPotionReward,
        RunAction::BuyShopCard { slot } => PublicChoice::BuyShopCard {
            shop_slot: public_slot(slot)?,
        },
        RunAction::BuyShopRelic { slot } => PublicChoice::BuyShopRelic {
            shop_slot: public_slot(slot)?,
        },
        RunAction::BuyShopPotion { slot } => PublicChoice::BuyShopPotion {
            shop_slot: public_slot(slot)?,
        },
        RunAction::UsePotion { slot, target } => {
            let target_slot = target
                .map(|id| {
                    run.combat
                        .as_ref()
                        .and_then(|combat| {
                            combat.monsters.iter().position(|monster| monster.id == id)
                        })
                        .ok_or(FairError::DecisionUnavailable)
                        .and_then(public_slot)
                })
                .transpose()?;
            PublicChoice::UsePotionSlot {
                potion_slot: public_slot(slot)?,
                target_slot,
            }
        }
        RunAction::DiscardPotion { slot } => PublicChoice::DiscardPotionSlot {
            potion_slot: public_slot(slot)?,
        },
        RunAction::ChooseCombatCardReward { index } => PublicChoice::ChooseVisibleOption {
            option_slot: public_slot(index)?,
        },
        RunAction::SkipCombatCardReward => PublicChoice::SkipSelection,
        RunAction::ChooseHandSelect { index }
        | RunAction::ChooseDrawSelect { index }
        | RunAction::ChooseDiscardSelect { index }
        | RunAction::ChooseExhaustSelect { index } => PublicChoice::ToggleVisibleCard {
            option_slot: public_slot(index)?,
        },
        RunAction::ConfirmHandSelect
        | RunAction::ConfirmDrawSelect
        | RunAction::ConfirmDiscardSelect
        | RunAction::ConfirmExhaustSelect => PublicChoice::ConfirmSelection,
        RunAction::ConfirmHandSelectWithoutRetrieval => {
            PublicChoice::ConfirmSelectionWithoutRetrieval
        }
        RunAction::EnterShop => PublicChoice::EnterShop,
        RunAction::LeaveShop => PublicChoice::LeaveShop,
        RunAction::OpenShopRemove => PublicChoice::OpenShopRemove,
    })
}

fn deck_slot(
    run: &RunState,
    card_id: sts_core::adapter_internals::CardId,
) -> Result<u16, FairError> {
    run.deck
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(FairError::DecisionUnavailable)
        .and_then(public_slot)
}

fn public_slot(index: usize) -> Result<u16, FairError> {
    u16::try_from(index).map_err(|_| FairError::DecisionUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts_core::adapter_internals::{
        combat::CombatPhase, CardId, MonsterId, Potion, Relic, RoomKind, RunPhase,
    };

    #[test]
    fn every_combat_legal_action_has_one_public_choice() {
        let run = RunState::combat_fixture();
        let legal =
            sts_core::adapter_internals::legal_run_decision_actions(&run).expect("legal actions");
        let projected = projected_choices(&run).expect("public choices");
        assert_eq!(projected.len(), legal.len());
        assert_eq!(
            projected
                .iter()
                .map(|(_, action)| *action)
                .collect::<Vec<_>>(),
            legal
        );
    }

    #[test]
    fn map_choices_use_node_slots_and_preserve_order() {
        let run = RunState::map_fixture();
        let projected = projected_choices(&run).expect("map choices");
        assert!(!projected.is_empty());
        assert!(projected
            .iter()
            .all(|(choice, _)| matches!(choice, PublicChoice::ChooseMapNode { .. })));
    }

    #[test]
    fn rest_choices_use_deck_slots_not_card_ids() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(RoomKind::Rest);
        let id = run.deck[0].id;
        let choices = projected_choices(&run).expect("rest choices");
        let json =
            serde_json::to_string(&choices.iter().map(|(choice, _)| choice).collect::<Vec<_>>())
                .expect("serializes");
        assert!(!json.contains("card_id"));
        assert!(!json.contains(&format!("card:{}", id.get())));
        assert!(choices
            .iter()
            .any(|(choice, _)| matches!(choice, PublicChoice::RestSmith { card_slot: 0 })));
    }

    #[test]
    fn combat_choices_ignore_internal_id_renumbering() {
        let first = RunState::combat_fixture();
        let mut second = first.clone();
        let combat = second.combat.as_mut().expect("combat");
        for card in combat
            .piles
            .hand
            .iter_mut()
            .chain(combat.piles.draw_pile.iter_mut())
            .chain(combat.piles.discard_pile.iter_mut())
            .chain(combat.piles.exhaust_pile.iter_mut())
        {
            card.id = CardId::new(card.id.get() + 10_000);
        }
        for monster in &mut combat.monsters {
            monster.id = MonsterId::new(monster.id.get() + 10_000);
        }
        let public = |run: &RunState| {
            projected_choices(run)
                .expect("choices")
                .into_iter()
                .map(|(choice, _)| choice)
                .collect::<Vec<_>>()
        };
        assert_eq!(public(&first), public(&second));
    }

    #[test]
    fn combat_choices_ignore_hidden_rng_and_draw_order() {
        let first = RunState::combat_fixture();
        let mut second = first.clone();
        second
            .combat
            .as_mut()
            .expect("combat")
            .piles
            .draw_pile
            .reverse();
        second.event_rng_seed = 123;
        second.reward_rng_seed = 456;
        let public = |run: &RunState| {
            projected_choices(run)
                .expect("choices")
                .into_iter()
                .map(|(choice, _)| choice)
                .collect::<Vec<_>>()
        };
        assert_eq!(public(&first), public(&second));
    }

    #[test]
    fn potion_choices_use_public_slots() {
        let mut run = RunState::combat_fixture();
        run.potions = vec![Potion::Energy, Potion::Fire];
        run.empty_potion_slots = vec![2];
        let choices = projected_choices(&run)
            .expect("choices")
            .into_iter()
            .map(|(choice, _)| choice)
            .collect::<Vec<_>>();
        assert!(choices.contains(&PublicChoice::UsePotionSlot {
            potion_slot: 0,
            target_slot: None
        }));
        assert!(choices.contains(&PublicChoice::UsePotionSlot {
            potion_slot: 1,
            target_slot: Some(0)
        }));
        assert!(choices.contains(&PublicChoice::DiscardPotionSlot { potion_slot: 1 }));
    }

    #[test]
    fn runic_dome_hidden_intent_does_not_change_choices() {
        let first = RunState::combat_fixture_with_relics(vec![Relic::RunicDome]);
        let mut second = first.clone();
        second.combat.as_mut().expect("combat").monsters[0].intent =
            sts_core::adapter_internals::MonsterIntent::Block { block: 99 };
        let public = |run: &RunState| {
            projected_choices(run)
                .expect("choices")
                .into_iter()
                .map(|(choice, _)| choice)
                .collect::<Vec<_>>()
        };
        assert_eq!(public(&first), public(&second));
    }

    #[test]
    fn malformed_state_collapses_to_decision_unavailable() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.piles.draw_pile[0].id = combat.piles.hand[0].id;
        assert_eq!(projected_choices(&run), Err(FairError::DecisionUnavailable));
    }

    #[test]
    fn lost_combat_projects_proceed() {
        let mut run = RunState::combat_fixture();
        let combat = run.combat.as_mut().expect("combat");
        combat.phase = CombatPhase::Lost;
        combat.player.hp = 0;
        let choices = projected_choices(&run).expect("choices");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].0, PublicChoice::Proceed);
    }

    #[test]
    fn revision_is_explicit_monotonic_public_data() {
        assert_eq!(DecisionRevision::new(7).get(), 7);
        assert_eq!(
            DecisionRevision::new(7).checked_next(),
            Some(DecisionRevision::new(8))
        );
        assert_eq!(DecisionRevision::new(u64::MAX).checked_next(), None);
    }
}
