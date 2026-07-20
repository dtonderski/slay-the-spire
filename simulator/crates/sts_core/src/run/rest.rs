use crate::{
    content::cards::{card_instance_is_upgradeable, upgrade_card_instance},
    relic::{GIRYA_MAX_LIFTS, REGAL_PILLOW_HEAL},
    Relic, RestAction, RewardContinuation, RunPhase, RunState, SimError, SimResult,
};

use super::grid::{open_rest_remove_grid, open_rest_smith_grid};
use super::reward::{roll_event_relic_reward, roll_pending_card_reward_choices};
use crate::RewardScreen;

pub const REST_HEAL_PERCENT: i32 = 30;

#[must_use]
pub fn rest_heal_amount(max_hp: i32) -> i32 {
    max_hp * REST_HEAL_PERCENT / 100
}

#[must_use]
pub fn can_smith(run: &RunState) -> bool {
    !run.relics.contains(&Relic::FusionHammer)
}

#[must_use]
pub fn can_remove_at_rest(run: &RunState) -> bool {
    run.relics.contains(&Relic::PeacePipe)
}

#[must_use]
pub fn can_lift(run: &RunState) -> bool {
    run.relics.contains(&Relic::Girya) && run.girya_lifts < GIRYA_MAX_LIFTS
}

#[must_use]
pub fn can_dig(run: &RunState) -> bool {
    run.relics.contains(&Relic::Shovel)
}

#[must_use]
pub fn legal_rest_actions(run: &RunState) -> Vec<RestAction> {
    if run.phase != RunPhase::Rest {
        return Vec::new();
    }

    if run.rest_room_complete {
        return vec![RestAction::Proceed];
    }

    let mut actions = Vec::new();
    if !run.relics.contains(&Relic::CoffeeDripper) {
        actions.push(RestAction::Heal);
    }
    let has_upgradeable = run.deck.iter().any(card_instance_is_upgradeable);
    if has_upgradeable && can_smith(run) {
        actions.push(RestAction::OpenSmith);
    }
    if can_remove_at_rest(run) && run.deck.iter().any(|card| !card.bottled) {
        actions.push(RestAction::OpenRemove);
    }
    if can_lift(run) {
        actions.push(RestAction::Lift);
    }
    if can_dig(run) {
        actions.push(RestAction::Dig);
    }
    for card in &run.deck {
        if can_remove_at_rest(run) {
            actions.push(RestAction::RemoveCard { card_id: card.id });
        }
        if card_instance_is_upgradeable(card) && can_smith(run) {
            actions.push(RestAction::Smith { card_id: card.id });
        }
    }
    if actions.is_empty() {
        vec![RestAction::Proceed]
    } else {
        actions
    }
}

pub fn validate_rest_action(run: &RunState, action: RestAction) -> SimResult<()> {
    run.validate()?;

    if run.phase != RunPhase::Rest {
        return Err(SimError::IllegalAction("rest actions require rest phase"));
    }

    match action {
        RestAction::Proceed if legal_rest_actions(run).contains(&action) => Ok(()),
        RestAction::Proceed => Err(SimError::IllegalAction("rest room is not complete")),
        _ if run.rest_room_complete => Err(SimError::IllegalAction("rest room is complete")),
        RestAction::Heal if run.relics.contains(&Relic::CoffeeDripper) => {
            Err(SimError::IllegalAction("heal is not available"))
        }
        RestAction::Heal if legal_rest_actions(run).contains(&action) => Ok(()),
        RestAction::Heal => Err(SimError::IllegalAction("heal is not available")),
        RestAction::OpenSmith if !can_smith(run) => {
            Err(SimError::IllegalAction("smith is not available"))
        }
        RestAction::OpenSmith if legal_rest_actions(run).contains(&action) => Ok(()),
        RestAction::OpenSmith => Err(SimError::IllegalAction("smith is not available")),
        RestAction::OpenRemove if !can_remove_at_rest(run) => {
            Err(SimError::IllegalAction("remove is not available"))
        }
        RestAction::OpenRemove if run.deck.iter().any(|card| !card.bottled) => Ok(()),
        RestAction::OpenRemove => Err(SimError::IllegalAction("remove is not available")),
        RestAction::Lift if can_lift(run) => Ok(()),
        RestAction::Lift => Err(SimError::IllegalAction("lift is not available")),
        RestAction::Dig if can_dig(run) => Ok(()),
        RestAction::Dig => Err(SimError::IllegalAction("dig is not available")),
        RestAction::Smith { card_id } => {
            if !can_smith(run) {
                return Err(SimError::IllegalAction("smith is not available"));
            }
            let card = run
                .deck
                .iter()
                .find(|card| card.id == card_id)
                .ok_or(SimError::UnknownCard(card_id))?;
            if card_instance_is_upgradeable(card) {
                Ok(())
            } else {
                Err(SimError::IllegalAction("card cannot be upgraded"))
            }
        }
        RestAction::RemoveCard { card_id } => {
            if !can_remove_at_rest(run) {
                return Err(SimError::IllegalAction("remove is not available"));
            }
            if run.deck.iter().any(|card| card.id == card_id) {
                Ok(())
            } else {
                Err(SimError::UnknownCard(card_id))
            }
        }
    }
}

pub fn apply_rest_action(run: &RunState, action: RestAction) -> SimResult<RunState> {
    validate_rest_action(run, action)?;

    let mut next = run.clone();
    match action {
        RestAction::Heal => {
            let mut heal = rest_heal_amount(next.player_max_hp);
            if next.relics.contains(&Relic::RegalPillow) {
                heal += REGAL_PILLOW_HEAL;
            }
            next.heal_player(heal);
            next.rest_room_complete = true;
            if next.relics.contains(&Relic::DreamCatcher) {
                next.phase = RunPhase::Reward;
                next.reward = Some(RewardScreen {
                    continuation: RewardContinuation::Rest,
                    choices: Vec::new(),
                    queued_card_rewards: Vec::new(),
                    gold_offer: 0,
                    stolen_gold_offer: 0,
                    potion_offer: None,
                    potion_offers: Vec::new(),
                    relic_offer: None,
                    relic_key_offer: None,
                    pending_relic_offer: None,
                    pending_relic_key_offer: None,
                    queued_relic_key_offers: Vec::new(),
                    boss_relic_choices: Vec::new(),
                    card_reward_active: false,
                    card_reward_pending: true,
                    pending_card_reward_count: 1,
                });
                roll_pending_card_reward_choices(&mut next);
                next.reward
                    .as_mut()
                    .expect("rest card reward")
                    .card_reward_active = true;
            }
        }
        RestAction::OpenSmith => {
            open_rest_smith_grid(&mut next);
        }
        RestAction::OpenRemove => {
            open_rest_remove_grid(&mut next);
        }
        RestAction::Lift => {
            next.girya_lifts += 1;
            next.rest_room_complete = true;
        }
        RestAction::Dig => {
            let act = next.current_act;
            let key = roll_event_relic_reward(&mut next, act);
            let relic_offer = Relic::from_key(key);
            next.phase = RunPhase::Reward;
            next.reward = Some(RewardScreen {
                continuation: RewardContinuation::None,
                choices: Vec::new(),
                queued_card_rewards: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers: Vec::new(),
                relic_offer,
                relic_key_offer: if relic_offer.is_some() {
                    None
                } else {
                    Some(key)
                },
                pending_relic_offer: None,
                pending_relic_key_offer: None,
                queued_relic_key_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_active: false,
                card_reward_pending: false,
                pending_card_reward_count: 0,
            });
        }
        RestAction::Smith { card_id } => {
            let upgraded_card = next
                .deck
                .iter()
                .find(|card| card.id == card_id)
                .and_then(|card| upgrade_card_instance(*card))
                .expect("smith validated before apply");
            for card in &mut next.deck {
                if card.id == card_id {
                    *card = upgraded_card;
                    break;
                }
            }
            next.rest_room_complete = true;
        }
        RestAction::RemoveCard { card_id } => {
            next.remove_deck_card(card_id)
                .expect("rest remove validated before apply");
            next.rest_room_complete = true;
        }
        RestAction::Proceed => {
            next.phase = RunPhase::Idle;
            next.rest_room_complete = false;
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        run::reward::apply_run_action, RoomKind, RunAction, Snapshot, SNAPSHOT_SCHEMA_VERSION,
    };

    #[test]
    fn dream_catcher_reward_returns_to_completed_rest_room() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(RoomKind::Rest);
        run.event = None;
        run.reward = None;
        run.player_hp = 40;
        run.relics.push(Relic::DreamCatcher);

        let reward = apply_rest_action(&run, RestAction::Heal)
            .expect("Dream Catcher heal opens a card reward");
        assert_eq!(reward.phase, RunPhase::Reward);
        assert!(reward.rest_room_complete);
        assert!(reward
            .reward
            .as_ref()
            .is_some_and(|reward| reward.card_reward_active));

        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: reward.clone(),
        };
        let json = snapshot.canonical_json().expect("snapshot serializes");
        let restored: Snapshot<RunState> =
            serde_json::from_str(&json).expect("snapshot deserializes");
        assert_eq!(restored, snapshot);
        assert_eq!(
            restored
                .state
                .reward
                .as_ref()
                .expect("restored Dream Catcher reward")
                .continuation,
            RewardContinuation::Rest
        );

        let card_id = reward
            .reward
            .as_ref()
            .and_then(|reward| reward.choices.first())
            .expect("Dream Catcher offers a card")
            .id;
        let deck_size = reward.deck.len();
        let taken = apply_run_action(&reward, RunAction::TakeCardReward { card_id })
            .expect("Dream Catcher reward can be taken");
        assert_eq!(taken.phase, RunPhase::Rest);
        assert!(taken.rest_room_complete);
        assert!(taken.reward.is_none());
        assert_eq!(taken.deck.len(), deck_size + 1);

        let settled = apply_run_action(&reward, RunAction::CloseCardReward)
            .expect("Dream Catcher reward can be skipped");
        assert_eq!(settled.phase, RunPhase::Rest);
        assert!(settled.rest_room_complete);
        assert!(settled.reward.is_none());
    }
}
