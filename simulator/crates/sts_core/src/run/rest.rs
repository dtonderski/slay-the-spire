use crate::{
    content::cards::upgrade_content_id, RestAction, RunPhase, RunState, SimError, SimResult,
};

pub const REST_HEAL_PERCENT: i32 = 30;

#[must_use]
pub fn rest_heal_amount(max_hp: i32) -> i32 {
    max_hp * REST_HEAL_PERCENT / 100
}

#[must_use]
pub fn legal_rest_actions(run: &RunState) -> Vec<RestAction> {
    if run.phase != RunPhase::Rest {
        return Vec::new();
    }

    let mut actions = vec![RestAction::Heal];
    for card in &run.deck {
        actions.push(RestAction::RemoveCard { card_id: card.id });
        if upgrade_content_id(card.content_id).is_some() {
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
        RestAction::Heal if legal_rest_actions(run).contains(&action) => Ok(()),
        RestAction::Heal => Err(SimError::IllegalAction("heal is not available")),
        RestAction::Smith { card_id } => {
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
            let heal = rest_heal_amount(next.player_max_hp);
            next.player_hp = (next.player_hp + heal).min(next.player_max_hp);
            next.phase = RunPhase::Idle;
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
            next.phase = RunPhase::Idle;
        }
        RestAction::RemoveCard { card_id } => {
            next.deck.retain(|card| card.id != card_id);
            next.phase = RunPhase::Idle;
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
        assert_eq!(after.phase, RunPhase::Idle);
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
        assert_eq!(after.phase, RunPhase::Idle);
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
        for card in &run.deck {
            expected.push(RestAction::RemoveCard { card_id: card.id });
            if upgrade_content_id(card.content_id).is_some() {
                expected.push(RestAction::Smith { card_id: card.id });
            }
        }
        assert_eq!(legal_rest_actions(&run), expected);
    }

    #[test]
    fn remove_card_drops_card_from_master_deck() {
        let mut run = RunState::map_fixture();
        run.phase = RunPhase::Rest;
        let strike_id = run.deck[0].id;
        let starting_len = run.deck.len();

        let after = apply_rest_action(&run, RestAction::RemoveCard { card_id: strike_id })
            .expect("remove applies");

        assert_eq!(after.deck.len(), starting_len - 1);
        assert!(!after.deck.iter().any(|card| card.id == strike_id));
        assert_eq!(after.phase, RunPhase::Idle);
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
