//! A dismissable dungeon map overlays the completed room; it is not room entry.
use super::event::MatchAndKeepState;
use super::{EventScreen, RewardScreen, RunState, ShopScreen, TreasureRoomState};
use crate::{RunPhase, SimError, SimResult};
use serde::{Deserialize, Serialize};

/// The room's existing screen payloads, suspended while the map owns input.
/// No player, deck, RNG, floor, or other world state is captured here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapRoomScreen {
    phase: RunPhase,
    event: Option<EventScreen>,
    match_and_keep: Option<MatchAndKeepState>,
    reward: Option<RewardScreen>,
    shop: Option<ShopScreen>,
    shop_merchant_open: bool,
    treasure_room: Option<TreasureRoomState>,
    rest_room_complete: bool,
    emerald_key_reward_available: bool,
}

/// PC ProceedButton opens map(false), retaining the room and previous screen.
/// Call this at the exit-button transition, instead of destroying its payloads.
pub(crate) fn open_completed_room_map(run: &mut RunState) {
    let room = MapRoomScreen {
        phase: run.phase,
        event: run.event.take(),
        match_and_keep: run.match_and_keep.take(),
        reward: run.reward.take(),
        shop: run.shop.take(),
        shop_merchant_open: std::mem::take(&mut run.shop_merchant_open),
        treasure_room: run.treasure_room.take(),
        rest_room_complete: std::mem::take(&mut run.rest_room_complete),
        emerald_key_reward_available: std::mem::take(&mut run.emerald_key_reward_available),
    };
    run.map_room_screen = Some(Box::new(room));
    run.phase = RunPhase::Idle;
}

pub(crate) fn validate_map_return(run: &RunState) -> SimResult<()> {
    if run.phase != RunPhase::Idle || run.map_room_screen.is_none() {
        return Err(SimError::IllegalAction("no dismissable room map is open"));
    }
    Ok(())
}

pub(crate) fn apply_validated_map_return(mut run: RunState) -> RunState {
    let room = run.map_room_screen.take().expect("validated map return");
    run.phase = room.phase;
    run.event = room.event;
    run.match_and_keep = room.match_and_keep;
    run.reward = room.reward;
    run.shop = room.shop;
    run.shop_merchant_open = room.shop_merchant_open;
    run.treasure_room = room.treasure_room;
    run.rest_room_complete = room.rest_room_complete;
    run.emerald_key_reward_available = room.emerald_key_reward_available;
    run
}

pub(crate) fn validate_suspended_room(run: &RunState) -> SimResult<()> {
    if let Some(room) = &run.map_room_screen {
        if run.phase != RunPhase::Idle
            || !matches!(
                room.phase,
                RunPhase::Event
                    | RunPhase::Reward
                    | RunPhase::Rest
                    | RunPhase::Shop
                    | RunPhase::Treasure
            )
        {
            return Err(SimError::InvalidState(
                "suspended room requires a completed-room map",
            ));
        }
        // Validate the suspended owner against the same current world, not a
        // stored world snapshot. Reactivation itself draws no RNG.
        let active = apply_validated_map_return(run.clone());
        active.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::{CardRewardFlow, RewardContinuation};
    use crate::snapshot::{restore_run_snapshot_json, Snapshot, SNAPSHOT_SCHEMA_VERSION};
    use crate::{
        adapter_internals::{
            apply_run_action, apply_run_decision_action, legal_run_decision_actions,
        },
        EventAction, RunAction, RunDecisionAction,
    };

    fn event_owner() -> RunState {
        let mut run = RunState::seeded_ironclad(7, 0);
        run.phase = RunPhase::Event;
        run.event = Some(crate::run::event::neow_screen_for_stage(&run, 2));
        run
    }

    fn reward_owner() -> RunState {
        let mut run = event_owner();
        run.phase = RunPhase::Reward;
        run.event = None;
        run.reward = Some(RewardScreen {
            continuation: RewardContinuation::None,
            choices: Vec::new(),
            queued_card_rewards: Vec::new(),
            gold_offer: 23,
            stolen_gold_offer: 0,
            potion_offer: None,
            potion_offers: Vec::new(),
            relic_offer: None,
            pending_relic_offer: None,
            queued_relic_offers: Vec::new(),
            boss_relic_choices: Vec::new(),
            card_reward_flow: CardRewardFlow::None,
        });
        run
    }

    #[test]
    fn neow_leave_and_return_preserve_owner_and_entire_world() {
        let run = event_owner();
        let map = apply_run_decision_action(
            &run,
            RunDecisionAction::Event(EventAction::Choose { choice_index: 0 }),
        )
        .unwrap();
        assert_eq!(map.phase, RunPhase::Idle);
        assert!(map.event.is_none());
        assert!(legal_run_decision_actions(&map)
            .unwrap()
            .contains(&RunDecisionAction::MapReturn));
        let returned = apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap();
        assert_eq!(returned, run);
        assert!(validate_map_return(&returned).is_err());
    }

    #[test]
    fn unclaimed_reward_survives_map_and_can_be_claimed_once() {
        let run = reward_owner();
        let map = apply_run_action(&run, RunAction::Proceed).unwrap();
        let returned = apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap();
        assert_eq!(returned, run);
        let claimed = apply_run_action(&returned, RunAction::TakeGoldReward).unwrap();
        assert_eq!(claimed.gold, run.gold + 23);
        assert!(apply_run_action(&claimed, RunAction::TakeGoldReward).is_err());
        let map = apply_run_action(&claimed, RunAction::Proceed).unwrap();
        let returned = apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap();
        assert_eq!(returned, claimed);
    }

    #[test]
    fn snapshot_restore_preserves_suspended_owner_and_future_actions() {
        let run = reward_owner();
        let map = apply_run_action(&run, RunAction::Proceed).unwrap();
        let snapshot = Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            state: map.clone(),
        };
        let json = snapshot.canonical_json().unwrap();
        let restored = restore_run_snapshot_json(&json).unwrap();
        assert_eq!(snapshot.hash().unwrap(), restored.hash().unwrap());
        assert_eq!(
            legal_run_decision_actions(&map).unwrap(),
            legal_run_decision_actions(&restored.state).unwrap()
        );
        assert_eq!(
            apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap(),
            apply_run_decision_action(&restored.state, RunDecisionAction::MapReturn).unwrap()
        );
    }

    #[test]
    fn merchant_stock_and_rng_survive_return_without_room_entry() {
        let mut run = event_owner();
        run.event = None;
        run.current_room_override = Some(crate::map::RoomKind::Shop);
        crate::run::shop::enter_shop_room(&mut run).unwrap();
        crate::run::shop::open_shop_merchant(&mut run).unwrap();
        let mut map = run.clone();
        crate::run::shop::leave_shop_room(&mut map);
        map.validate().unwrap();
        let returned = apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap();
        assert_eq!(returned, run);
    }

    #[test]
    fn completed_rest_cannot_heal_again_after_return() {
        let mut run = event_owner();
        run.event = None;
        run.phase = RunPhase::Rest;
        run.current_room_override = Some(crate::map::RoomKind::Rest);
        run.rest_room_complete = true;
        let map =
            apply_run_decision_action(&run, RunDecisionAction::Rest(crate::RestAction::Proceed))
                .unwrap();
        let returned = apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap();
        assert_eq!(returned, run);
        assert_eq!(
            legal_run_decision_actions(&returned).unwrap(),
            vec![RunDecisionAction::Rest(crate::RestAction::Proceed)]
        );
    }

    #[test]
    fn beggar_purge_preserves_completed_owner_and_removes_only_once() {
        let mut run = event_owner();
        run.current_room_override = Some(crate::map::RoomKind::Event);
        run.event = Some(EventScreen {
            event: crate::run::event::Event::Beggar,
            choices: vec![crate::run::event::EventChoice {
                label: "Leave".to_owned(),
            }],
            stage: 2,
            event_data: 0,
        });
        crate::run::grid::open_event_remove_grid(&mut run);
        let selected =
            apply_run_decision_action(&run, RunDecisionAction::GridSelect { index: 0 }).unwrap();
        let removed_id = selected.card_grid.as_ref().unwrap().cards[0].id;
        let map = apply_run_decision_action(&selected, RunDecisionAction::GridConfirm).unwrap();
        assert!(map.deck.iter().all(|card| card.id != removed_id));
        let returned = apply_run_decision_action(&map, RunDecisionAction::MapReturn).unwrap();
        let mut expected = map.clone();
        expected.map_room_screen = None;
        expected.phase = RunPhase::Event;
        expected.event = run.event;
        assert_eq!(returned, expected);
        assert_eq!(returned.deck.len(), run.deck.len() - 1);
        assert_eq!(
            legal_run_decision_actions(&returned).unwrap(),
            vec![RunDecisionAction::Event(EventAction::Choose {
                choice_index: 0
            })]
        );
    }

    #[test]
    fn actual_node_entry_discards_suspended_owner() {
        let run = event_owner();
        let map = apply_run_decision_action(
            &run,
            RunDecisionAction::Event(EventAction::Choose { choice_index: 0 }),
        )
        .unwrap();
        let action = legal_run_decision_actions(&map)
            .unwrap()
            .into_iter()
            .find(|action| matches!(action, RunDecisionAction::Map(_)))
            .unwrap();
        let entered = apply_run_decision_action(&map, action).unwrap();
        assert!(entered.map_room_screen.is_none());
        assert!(entered.event.is_none());
        assert_eq!(entered.current_floor, run.current_floor + 1);
        assert!(validate_map_return(&entered).is_err());
    }

    #[test]
    fn first_map_and_terminal_states_have_no_return() {
        let run = RunState::map_fixture();
        assert!(validate_map_return(&run).is_err());
        let mut map = event_owner();
        open_completed_room_map(&mut map);
        map.phase = RunPhase::Complete;
        assert!(map.validate().is_err());
        assert!(validate_map_return(&map).is_err());
    }
}
