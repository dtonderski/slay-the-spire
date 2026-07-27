use crate::{
    content::cards::{card_instance_is_upgradeable, upgrade_card_instance},
    relic::{GIRYA_MAX_LIFTS, REGAL_PILLOW_HEAL},
    Relic, RestAction, RewardContinuation, RunPhase, RunState, SimError, SimResult,
};

use super::grid::{open_rest_remove_grid, open_rest_smith_grid};
use super::reward::{roll_pending_card_reward_choices, roll_reward_screen_relic};
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

pub fn legal_rest_actions(run: &RunState) -> SimResult<Vec<RestAction>> {
    run.validate()?;
    if run.phase != RunPhase::Rest {
        return Ok(Vec::new());
    }

    if run.rest_room_complete {
        return Ok(vec![RestAction::Proceed]);
    }

    let mut actions = Vec::new();
    // CampfireUI.initializeButtons: RestOption, then SmithOption, then each relic's
    // addCampfireOption in player relic inventory order (CommunicationMod filters
    // unusable buttons, preserving relative order among usable options).
    if !run.relics.contains(&Relic::CoffeeDripper) {
        actions.push(RestAction::Heal);
    }
    let has_upgradeable = run.deck.iter().any(card_instance_is_upgradeable);
    if has_upgradeable && can_smith(run) {
        actions.push(RestAction::OpenSmith);
    }
    let has_removable = run.deck.iter().any(|card| !card.bottled);
    for relic in &run.relics {
        match relic {
            Relic::PeacePipe if has_removable => {
                actions.push(RestAction::OpenRemove);
            }
            Relic::Girya if can_lift(run) => {
                actions.push(RestAction::Lift);
            }
            Relic::Shovel => {
                actions.push(RestAction::Dig);
            }
            _ => {}
        }
    }
    for card in &run.deck {
        if can_remove_at_rest(run) && !card.bottled {
            actions.push(RestAction::RemoveCard { card_id: card.id });
        }
        if card_instance_is_upgradeable(card) && can_smith(run) {
            actions.push(RestAction::Smith { card_id: card.id });
        }
    }
    if actions.is_empty() {
        Ok(vec![RestAction::Proceed])
    } else {
        Ok(actions)
    }
}

pub fn validate_rest_action(run: &RunState, action: RestAction) -> SimResult<()> {
    run.validate()?;

    if run.phase != RunPhase::Rest {
        return Err(SimError::IllegalAction("rest actions require rest phase"));
    }
    let legal_actions = legal_rest_actions(run)?;

    match action {
        RestAction::Proceed if legal_actions.contains(&action) => Ok(()),
        RestAction::Proceed => Err(SimError::IllegalAction("rest room is not complete")),
        _ if run.rest_room_complete => Err(SimError::IllegalAction("rest room is complete")),
        RestAction::Heal if run.relics.contains(&Relic::CoffeeDripper) => {
            Err(SimError::IllegalAction("heal is not available"))
        }
        RestAction::Heal if legal_actions.contains(&action) => Ok(()),
        RestAction::Heal => Err(SimError::IllegalAction("heal is not available")),
        RestAction::OpenSmith if !can_smith(run) => {
            Err(SimError::IllegalAction("smith is not available"))
        }
        RestAction::OpenSmith if legal_actions.contains(&action) => Ok(()),
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
            next.heal_player(heal)?;
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
                    pending_relic_offer: None,
                    queued_relic_offers: Vec::new(),
                    boss_relic_choices: Vec::new(),
                    card_reward_flow: crate::run::CardRewardFlow::pending(1),
                });
                roll_pending_card_reward_choices(&mut next)?;
                next.reward
                    .as_mut()
                    .expect("rest card reward")
                    .open_card_reward()?;
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
            // CampfireDigEffect: returnRandomRelicTier() + returnRandomRelic(tier)
            // (not returnRandomScreenlessRelic — Dig opens CombatRewardScreen).
            let act = next.current_act;
            let key = roll_reward_screen_relic(&mut next, act);
            next.rest_room_complete = true;
            next.phase = RunPhase::Reward;
            next.reward = Some(RewardScreen {
                continuation: RewardContinuation::Rest,
                choices: Vec::new(),
                queued_card_rewards: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
                potion_offers: Vec::new(),
                relic_offer: Some(key),
                pending_relic_offer: None,
                queued_relic_offers: Vec::new(),
                boss_relic_choices: Vec::new(),
                card_reward_flow: crate::run::CardRewardFlow::None,
            });
        }
        RestAction::Smith { card_id } => {
            let card = next
                .deck
                .iter()
                .find(|card| card.id == card_id)
                .copied()
                .ok_or(SimError::UnknownCard(card_id))?;
            let upgraded_card = upgrade_card_instance(card)?
                .ok_or(SimError::IllegalAction("card cannot be upgraded"))?;
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
        content::cards::{ANGER_ID, SEARING_BLOW_PLUS_ID},
        run::reward::{apply_run_action, roll_event_relic_reward, roll_reward_screen_relic},
        CardId, CardInstance, RoomKind, RunAction, Snapshot, SNAPSHOT_SCHEMA_VERSION,
    };

    #[test]
    fn max_searing_blow_upgrade_is_legal_but_fails_closed_when_applied() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(RoomKind::Rest);
        run.event = None;
        let mut searing_blow = CardInstance::new(CardId::new(100), SEARING_BLOW_PLUS_ID);
        searing_blow.searing_blow_upgrades = u8::MAX;
        run.deck = vec![searing_blow];
        run.validate()
            .expect("maximum modeled Searing Blow is valid");

        let action = RestAction::Smith {
            card_id: searing_blow.id,
        };
        assert!(legal_rest_actions(&run)
            .expect("rest actions are available")
            .contains(&action));
        assert_eq!(
            apply_rest_action(&run, action),
            Err(SimError::InvalidState(
                "Searing Blow upgrade count overflows u8"
            ))
        );
        assert_eq!(run.deck, vec![searing_blow]);
    }

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
            .is_some_and(RewardScreen::card_reward_is_active));

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

    #[test]
    fn relic_campfire_options_follow_player_relic_inventory_order() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(RoomKind::Rest);
        run.event = None;
        run.relics
            .extend([Relic::Shovel, Relic::Girya, Relic::PeacePipe]);

        let actions = legal_rest_actions(&run).expect("rest actions");
        let screen: Vec<_> = actions
            .into_iter()
            .filter(|action| {
                matches!(
                    action,
                    RestAction::Heal
                        | RestAction::OpenSmith
                        | RestAction::OpenRemove
                        | RestAction::Lift
                        | RestAction::Dig
                )
            })
            .collect();
        assert_eq!(
            screen,
            vec![
                RestAction::Heal,
                RestAction::OpenSmith,
                RestAction::Dig,
                RestAction::Lift,
                RestAction::OpenRemove,
            ]
        );

        // Reverse relic order must reverse dig/lift/toke relative order.
        run.relics
            .retain(|relic| !matches!(relic, Relic::Shovel | Relic::Girya | Relic::PeacePipe));
        run.relics
            .extend([Relic::PeacePipe, Relic::Girya, Relic::Shovel]);
        let actions = legal_rest_actions(&run).expect("rest actions");
        let screen: Vec<_> = actions
            .into_iter()
            .filter(|action| {
                matches!(
                    action,
                    RestAction::Heal
                        | RestAction::OpenSmith
                        | RestAction::OpenRemove
                        | RestAction::Lift
                        | RestAction::Dig
                )
            })
            .collect();
        assert_eq!(
            screen,
            vec![
                RestAction::Heal,
                RestAction::OpenSmith,
                RestAction::OpenRemove,
                RestAction::Lift,
                RestAction::Dig,
            ]
        );
    }

    #[test]
    fn shovel_reward_returns_to_completed_rest_room_after_relic_pickup() {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(RoomKind::Rest);
        run.event = None;
        run.relics.push(Relic::Shovel);

        let reward =
            apply_rest_action(&run, RestAction::Dig).expect("Shovel dig opens a relic reward");
        assert_eq!(reward.phase, RunPhase::Reward);
        assert!(reward.rest_room_complete);
        assert_eq!(
            reward.reward.as_ref().expect("dig reward").continuation,
            RewardContinuation::Rest
        );

        let claimed = apply_run_action(&reward, RunAction::TakeRelicReward)
            .expect("Shovel relic can be collected");
        assert_eq!(claimed.phase, RunPhase::Reward);
        let settled = apply_run_action(&claimed, RunAction::Proceed)
            .expect("completed Shovel reward returns to rest");
        assert_eq!(settled.phase, RunPhase::Rest);
        assert!(settled.rest_room_complete);
        assert!(settled.reward.is_none());
    }

    #[test]
    fn dig_uses_reward_screen_relic_not_screenless_event_roll() {
        // CampfireDigEffect uses returnRandomRelicTier + returnRandomRelic, so
        // bottled relics may appear. Event instant-grants use screenless and skip
        // bottles/Whetstone.
        let mut base = RunState::seeded_ironclad(42, 0);
        base.phase = RunPhase::Rest;
        base.current_room_override = Some(RoomKind::Rest);
        base.event = None;
        base.relics.push(Relic::Shovel);
        base.deck
            .push(CardInstance::new(CardId::new(500), ANGER_ID));
        base.ensure_ironclad_relic_pools();
        {
            let pools = base.relic_pools.as_mut().expect("pools");
            pools.remove_relic(Relic::BottledFlame);
            pools.remove_relic(Relic::MeatOnTheBone);
            for pool in [&mut pools.common, &mut pools.uncommon, &mut pools.rare] {
                pool.insert(0, Relic::BottledFlame);
                pool.insert(1, Relic::MeatOnTheBone);
            }
        }

        let dig = apply_rest_action(&base, RestAction::Dig).expect("dig opens reward");
        let dig_offer = dig
            .reward
            .as_ref()
            .expect("dig reward")
            .relic_offer
            .expect("dig relic offer");

        let act = base.current_act;
        let mut screen_run = base.clone();
        let screen = roll_reward_screen_relic(&mut screen_run, act);
        let mut event_run = base.clone();
        let event = roll_event_relic_reward(&mut event_run, act);

        assert_eq!(dig_offer, screen);
        assert_eq!(screen, Relic::BottledFlame);
        assert_eq!(event, Relic::MeatOnTheBone);
    }
}
