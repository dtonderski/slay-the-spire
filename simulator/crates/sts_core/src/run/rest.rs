use crate::{
    content::cards::upgrade_content_id,
    relic::{GIRYA_MAX_LIFTS, REGAL_PILLOW_HEAL},
    Relic, RestAction, RunPhase, RunState, SimError, SimResult,
};

use super::grid::open_rest_smith_grid;
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
    let has_upgradeable = run
        .deck
        .iter()
        .any(|card| upgrade_content_id(card.content_id).is_some());
    if has_upgradeable && can_smith(run) {
        actions.push(RestAction::OpenSmith);
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
        if upgrade_content_id(card.content_id).is_some() && can_smith(run) {
            actions.push(RestAction::Smith { card_id: card.id });
        }
    }
    actions
}

pub fn validate_rest_action(run: &RunState, action: RestAction) -> SimResult<()> {
    if run.phase != RunPhase::Rest {
        return Err(SimError::IllegalAction("rest actions require rest phase"));
    }

    match action {
        RestAction::Proceed if run.rest_room_complete => Ok(()),
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
            if upgrade_content_id(card.content_id).is_some() {
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
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            if next.relics.contains(&Relic::DreamCatcher) {
                next.phase = RunPhase::Reward;
                next.reward = Some(RewardScreen {
                    choices: Vec::new(),
                    gold_offer: 0,
                    stolen_gold_offer: 0,
                    potion_offer: None,
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
            } else {
                next.rest_room_complete = true;
            }
        }
        RestAction::OpenSmith => {
            open_rest_smith_grid(&mut next);
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
                choices: Vec::new(),
                gold_offer: 0,
                stolen_gold_offer: 0,
                potion_offer: None,
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
            let upgraded_content_id = next
                .deck
                .iter()
                .find(|card| card.id == card_id)
                .and_then(|card| upgrade_content_id(card.content_id))
                .expect("smith validated before apply");
            for card in &mut next.deck {
                if card.id == card_id {
                    card.content_id = upgraded_content_id;
                    break;
                }
            }
            next.rest_room_complete = true;
        }
        RestAction::RemoveCard { card_id } => {
            next.deck.retain(|card| card.id != card_id);
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
        content::cards::{STRIKE_R_ID, STRIKE_R_PLUS_ID},
        content::character::IRONCLAD_A0_BASE_HP,
        map::RoomKind,
        run::grid::GridPurpose,
        RunState,
    };

    #[test]
    fn rest_heal_amount_floors_thirty_percent_of_max_hp() {
        assert_eq!(rest_heal_amount(80), 24);
        assert_eq!(rest_heal_amount(79), 23);
    }

    #[test]
    fn heal_caps_at_max_hp() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = IRONCLAD_A0_BASE_HP - 10;

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(after.player_hp, IRONCLAD_A0_BASE_HP);
        assert_eq!(after.phase, RunPhase::Rest);
        assert!(after.rest_room_complete);
    }

    #[test]
    fn heal_does_not_exceed_max_hp_when_near_full() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = IRONCLAD_A0_BASE_HP - 5;

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(after.player_hp, IRONCLAD_A0_BASE_HP);
    }

    #[test]
    fn regal_pillow_adds_rest_healing() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = 20;
        run.relics.push(Relic::RegalPillow);

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(
            after.player_hp,
            20 + rest_heal_amount(run.player_max_hp) + REGAL_PILLOW_HEAL
        );
    }

    #[test]
    fn eternal_feather_does_not_add_extra_healing_when_resting() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = 20;
        run.relics.push(Relic::EternalFeather);

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(after.player_hp, 20 + rest_heal_amount(run.player_max_hp));
    }

    #[test]
    fn dream_catcher_modeled_relic_opens_card_reward_after_rest() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.player_hp = 20;
        run.relics.push(Relic::DreamCatcher);

        let after = apply_rest_action(&run, RestAction::Heal).expect("heal applies");

        assert_eq!(after.phase, RunPhase::Reward);
        assert!(
            after
                .reward
                .as_ref()
                .expect("dream catcher reward")
                .card_reward_active
        );
    }

    #[test]
    fn rest_action_is_illegal_outside_rest_phase() {
        let run = RunState::map_fixture();

        let err = apply_rest_action(&run, RestAction::Heal).expect_err("not at rest");

        assert_eq!(
            err,
            SimError::IllegalAction("rest actions require rest phase")
        );
    }

    #[test]
    fn smith_upgrades_strike_in_master_deck() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        let strike_id = run.deck[0].id;
        assert_eq!(run.deck[0].content_id, STRIKE_R_ID);

        let after = apply_rest_action(&run, RestAction::Smith { card_id: strike_id })
            .expect("smith applies");

        assert_eq!(after.deck[0].content_id, STRIKE_R_PLUS_ID);
        assert_eq!(after.deck[0].id, strike_id);
        assert_eq!(after.count_content_in_deck(STRIKE_R_ID), 4);
        assert_eq!(after.count_content_in_deck(STRIKE_R_PLUS_ID), 1);
        assert_eq!(after.phase, RunPhase::Rest);
        assert!(after.rest_room_complete);
    }

    #[test]
    fn smith_is_illegal_outside_rest_phase() {
        let run = RunState::map_fixture();
        let strike_id = run.deck[0].id;

        let err = apply_rest_action(&run, RestAction::Smith { card_id: strike_id })
            .expect_err("not at rest");

        assert_eq!(
            err,
            SimError::IllegalAction("rest actions require rest phase")
        );
    }

    #[test]
    fn smith_is_illegal_for_already_upgraded_card() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.deck[0].content_id = STRIKE_R_PLUS_ID;
        let strike_id = run.deck[0].id;

        let err = apply_rest_action(&run, RestAction::Smith { card_id: strike_id })
            .expect_err("already upgraded");

        assert_eq!(err, SimError::IllegalAction("card cannot be upgraded"));
    }

    #[test]
    fn fusion_hammer_disables_smith_actions_but_keeps_rest_and_peace_pipe_remove() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.relics.extend([Relic::FusionHammer, Relic::PeacePipe]);
        let strike_id = run.deck[0].id;

        let actions = legal_rest_actions(&run);

        assert!(actions.contains(&RestAction::Heal));
        assert!(actions.contains(&RestAction::RemoveCard { card_id: strike_id }));
        assert!(!actions.contains(&RestAction::OpenSmith));
        assert!(!actions.contains(&RestAction::Smith { card_id: strike_id }));
    }

    #[test]
    fn fusion_hammer_rejects_direct_smith_actions() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.relics.push(Relic::FusionHammer);
        let strike_id = run.deck[0].id;

        let open_err = apply_rest_action(&run, RestAction::OpenSmith).expect_err("open blocked");
        let smith_err = apply_rest_action(&run, RestAction::Smith { card_id: strike_id })
            .expect_err("smith blocked");

        assert_eq!(open_err, SimError::IllegalAction("smith is not available"));
        assert_eq!(smith_err, SimError::IllegalAction("smith is not available"));
    }

    #[test]
    fn entering_rest_room_exposes_heal_and_smith_actions() {
        use crate::{apply_map_action_on_run, legal_rest_actions, MapAction, MapNodeId};

        let mut run = RunState::map_fixture();
        run.player_hp = 40;

        run = apply_map_action_on_run(
            &run,
            MapAction::ChooseNode {
                node_id: MapNodeId::new(2),
            },
        )
        .expect("enter rest room");

        assert_eq!(run.phase, RunPhase::Rest);
        assert_eq!(
            run.map
                .as_ref()
                .and_then(|map| map.map.node(map.current_node))
                .map(|node| node.room_kind),
            Some(RoomKind::Rest)
        );

        let mut expected = vec![RestAction::Heal];
        if run
            .deck
            .iter()
            .any(|card| upgrade_content_id(card.content_id).is_some())
        {
            expected.push(RestAction::OpenSmith);
        }
        for card in &run.deck {
            if upgrade_content_id(card.content_id).is_some() {
                expected.push(RestAction::Smith { card_id: card.id });
            }
        }
        assert_eq!(legal_rest_actions(&run), expected);
    }

    #[test]
    fn rest_actions_transition_to_expected_phase_reward_or_grid_destinations() {
        let mut base = RunState::map_fixture();
        base.phase = RunPhase::Rest;
        base.player_hp = 20;
        base.relics
            .extend([Relic::PeacePipe, Relic::Girya, Relic::Shovel]);
        let strike_id = base.deck[0].id;

        let healed = apply_rest_action(&base, RestAction::Heal).expect("heal");
        assert_eq!(healed.phase, RunPhase::Rest);
        assert!(healed.rest_room_complete);
        assert!(healed.reward.is_none());
        assert!(healed.card_grid.is_none());
        let proceeded = apply_rest_action(&healed, RestAction::Proceed).expect("proceed");
        assert_eq!(proceeded.phase, RunPhase::Idle);
        assert!(!proceeded.rest_room_complete);

        let smith_grid = apply_rest_action(&base, RestAction::OpenSmith).expect("open smith");
        assert_eq!(smith_grid.phase, RunPhase::Rest);
        assert_eq!(
            smith_grid.card_grid.as_ref().map(|grid| grid.purpose),
            Some(GridPurpose::RestSmith)
        );
        assert!(smith_grid.reward.is_none());

        let removed = apply_rest_action(&base, RestAction::RemoveCard { card_id: strike_id })
            .expect("peace pipe remove");
        assert_eq!(removed.phase, RunPhase::Rest);
        assert!(removed.rest_room_complete);
        assert!(removed.reward.is_none());
        assert!(removed.card_grid.is_none());

        let lifted = apply_rest_action(&base, RestAction::Lift).expect("girya lift");
        assert_eq!(lifted.phase, RunPhase::Rest);
        assert!(lifted.rest_room_complete);
        assert_eq!(lifted.girya_lifts, base.girya_lifts + 1);
        assert!(lifted.reward.is_none());
        assert!(lifted.card_grid.is_none());

        let dug = apply_rest_action(&base, RestAction::Dig).expect("shovel dig");
        assert_eq!(dug.phase, RunPhase::Reward);
        assert!(dug.card_grid.is_none());
        assert!(dug.reward.as_ref().is_some_and(|reward| {
            reward.relic_offer.is_some() || reward.relic_key_offer.is_some()
        }));
    }

    #[test]
    fn remove_card_without_peace_pipe_is_illegal() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        let strike_id = run.deck[0].id;

        let err = apply_rest_action(&run, RestAction::RemoveCard { card_id: strike_id })
            .expect_err("remove blocked");

        assert_eq!(err, SimError::IllegalAction("remove is not available"));
    }

    #[test]
    fn peace_pipe_remove_card_drops_card_from_master_deck() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.relics.push(Relic::PeacePipe);
        let strike_id = run.deck[0].id;
        let starting_len = run.deck.len();

        let after = apply_rest_action(&run, RestAction::RemoveCard { card_id: strike_id })
            .expect("remove applies");

        assert_eq!(after.deck.len(), starting_len - 1);
        assert!(!after.deck.iter().any(|card| card.id == strike_id));
        assert_eq!(after.phase, RunPhase::Rest);
        assert!(after.rest_room_complete);
    }

    #[test]
    fn girya_lift_increments_lift_count_and_leaves_rest() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.relics.push(Relic::Girya);

        assert!(legal_rest_actions(&run).contains(&RestAction::Lift));

        let after = apply_rest_action(&run, RestAction::Lift).expect("lift applies");

        assert_eq!(after.girya_lifts, 1);
        assert_eq!(after.phase, RunPhase::Rest);
        assert!(after.rest_room_complete);
    }

    #[test]
    fn girya_lift_is_illegal_without_relic_or_after_three_lifts() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;

        let missing_relic = apply_rest_action(&run, RestAction::Lift).expect_err("no girya");
        assert_eq!(
            missing_relic,
            SimError::IllegalAction("lift is not available")
        );

        run.relics.push(Relic::Girya);
        run.girya_lifts = GIRYA_MAX_LIFTS;

        let capped = apply_rest_action(&run, RestAction::Lift).expect_err("capped");
        assert_eq!(capped, SimError::IllegalAction("lift is not available"));
    }

    #[test]
    fn shovel_dig_opens_relic_reward_screen() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        run.relics.push(Relic::Shovel);
        let relic_counter_before = run.relic_rng_counter;

        assert!(legal_rest_actions(&run).contains(&RestAction::Dig));

        let after = apply_rest_action(&run, RestAction::Dig).expect("dig applies");
        let reward = after.reward.as_ref().expect("dig reward");

        assert_eq!(after.phase, RunPhase::Reward);
        assert!(reward.relic_offer.is_some() || reward.relic_key_offer.is_some());
        assert!(reward.choices.is_empty());
        assert_eq!(reward.gold_offer, 0);
        assert!(reward.potion_offer.is_none());
        assert!(!reward.card_reward_active);
        assert!(!reward.card_reward_pending);
        assert!(after.relic_rng_counter > relic_counter_before);
    }

    #[test]
    fn shovel_dig_is_illegal_without_relic() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;

        let err = apply_rest_action(&run, RestAction::Dig).expect_err("no shovel");

        assert_eq!(err, SimError::IllegalAction("dig is not available"));
        assert!(!legal_rest_actions(&run).contains(&RestAction::Dig));
    }

    #[test]
    fn remove_card_is_illegal_outside_rest_phase() {
        let run = RunState::map_fixture();
        let strike_id = run.deck[0].id;

        let err = apply_rest_action(&run, RestAction::RemoveCard { card_id: strike_id })
            .expect_err("not at rest");

        assert_eq!(
            err,
            SimError::IllegalAction("rest actions require rest phase")
        );
    }
}
