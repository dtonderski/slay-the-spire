use crate::{
    action::{CardPile, CombatAction, InternalAction},
    combat::{damage::deal_unmodified_damage_to_monster, validate_combat_action, CombatPhase},
    content::cards::{get_card_definition, BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    ids::{CardId, MonsterId},
    CardInstance, CombatState, MonsterState, SimError, SimResult,
};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatTransition {
    pub state: CombatState,
    pub event_log: Vec<InternalAction>,
}

pub fn apply_combat_action(state: &CombatState, action: CombatAction) -> SimResult<CombatState> {
    Ok(apply_combat_action_with_events(state, action)?.state)
}

pub fn apply_combat_action_with_events(
    state: &CombatState,
    action: CombatAction,
) -> SimResult<CombatTransition> {
    validate_combat_action(state, action)?;

    match action {
        CombatAction::PlayCard { card_id, target } => apply_play_card(state, card_id, target),
        CombatAction::EndTurn => Ok(CombatTransition {
            state: crate::combat::end_player_turn(state),
            event_log: Vec::new(),
        }),
    }
}

fn apply_play_card(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
) -> SimResult<CombatTransition> {
    let card = find_hand_card(state, card_id)?;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    let queue = match definition.id {
        STRIKE_R_ID => strike_queue(card_id, target.expect("validated Strike has a target")),
        DEFEND_R_ID => defend_queue(card_id),
        BASH_ID => bash_queue(card_id, target.expect("validated Bash has a target")),
        _ => Err(SimError::IllegalAction(
            "card transition is not implemented",
        ))?,
    };

    process_internal_queue(state, queue?)
}

fn strike_queue(card_id: CardId, target: MonsterId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::DealDamage { target, amount: 6 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn defend_queue(card_id: CardId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 1 },
        InternalAction::GainBlock { amount: 5 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn bash_queue(card_id: CardId, target: MonsterId) -> SimResult<VecDeque<InternalAction>> {
    Ok(VecDeque::from([
        InternalAction::PlayCard { card_id },
        InternalAction::SpendEnergy { amount: 2 },
        InternalAction::DealDamage { target, amount: 8 },
        InternalAction::ApplyVulnerable { target, amount: 2 },
        InternalAction::MoveCard {
            card_id,
            from: CardPile::Hand,
            to: CardPile::DiscardPile,
        },
    ]))
}

fn process_internal_queue(
    state: &CombatState,
    mut queue: VecDeque<InternalAction>,
) -> SimResult<CombatTransition> {
    let mut next = state.clone();
    let mut event_log = Vec::new();

    while let Some(internal_action) = queue.pop_front() {
        apply_internal_action(&mut next, internal_action)?;
        event_log.push(internal_action);
    }

    if next.monsters.iter().all(|monster| !monster.alive) {
        next.phase = CombatPhase::Won;
    } else {
        next.phase = CombatPhase::WaitingForPlayer;
    }

    Ok(CombatTransition {
        state: next,
        event_log,
    })
}

fn apply_internal_action(state: &mut CombatState, action: InternalAction) -> SimResult<()> {
    match action {
        InternalAction::PlayCard { .. } => Ok(()),
        InternalAction::SpendEnergy { amount } => {
            state.player.energy -= amount;
            Ok(())
        }
        InternalAction::DealDamage { target, amount } => {
            let monster = living_monster_mut(state, target)?;
            deal_unmodified_damage_to_monster(monster, amount);
            Ok(())
        }
        InternalAction::GainBlock { amount } => {
            state.player.block += amount;
            Ok(())
        }
        InternalAction::ApplyVulnerable { target, amount } => {
            let monster = living_monster_mut(state, target)?;
            monster.powers.vulnerable += amount;
            Ok(())
        }
        InternalAction::MoveCard { card_id, from, to } => move_card(state, card_id, from, to),
    }
}

fn living_monster_mut(state: &mut CombatState, target: MonsterId) -> SimResult<&mut MonsterState> {
    state
        .monsters
        .iter_mut()
        .find(|monster| monster.id == target && monster.alive)
        .ok_or(SimError::IllegalAction("target is not a living monster"))
}

fn find_hand_card(state: &CombatState, card_id: CardId) -> SimResult<CardInstance> {
    state
        .piles
        .hand
        .iter()
        .copied()
        .find(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))
}

fn remove_card_from_hand(state: &mut CombatState, card_id: CardId) -> SimResult<CardInstance> {
    let index = state
        .piles
        .hand
        .iter()
        .position(|card| card.id == card_id)
        .ok_or(SimError::UnknownCard(card_id))?;

    Ok(state.piles.hand.remove(index))
}

fn move_card(
    state: &mut CombatState,
    card_id: CardId,
    from: CardPile,
    to: CardPile,
) -> SimResult<()> {
    let card = match from {
        CardPile::Hand => remove_card_from_hand(state, card_id)?,
        CardPile::DrawPile | CardPile::DiscardPile | CardPile::ExhaustPile => {
            return Err(SimError::IllegalAction(
                "card move source is not implemented",
            ));
        }
    };

    match to {
        CardPile::DiscardPile => {
            state.piles.discard_pile.push(card);
            Ok(())
        }
        CardPile::Hand | CardPile::DrawPile | CardPile::ExhaustPile => Err(
            SimError::IllegalAction("card move destination is not implemented"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID};

    #[test]
    fn strike_decreases_monster_hp_by_six() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 6);
    }

    #[test]
    fn strike_decreases_energy_by_one() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn strike_moves_card_from_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let strike_id = hand_strike_id(&state);

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == strike_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == strike_id));
    }

    #[test]
    fn strike_can_win_combat() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 6;

        let next = apply_combat_action(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(next.phase, CombatPhase::Won);
        assert!(!next.monsters[0].alive);
    }

    #[test]
    fn invalid_target_returns_error_and_preserves_state() {
        let state = CombatState::initial_fixture();
        let before = state.snapshot().hash().expect("state hashes before");

        let result = apply_combat_action(
            &state,
            CombatAction::PlayCard {
                card_id: hand_strike_id(&state),
                target: Some(MonsterId::new(99)),
            },
        );

        assert_eq!(
            result,
            Err(SimError::IllegalAction("target is not a living monster"))
        );
        assert_eq!(state.snapshot().hash().expect("state hashes after"), before);
    }

    #[test]
    fn defend_increases_player_block_by_five() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.block, state.player.block + 5);
    }

    #[test]
    fn defend_decreases_energy_by_one() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(next.player.energy, state.player.energy - 1);
    }

    #[test]
    fn defend_moves_card_from_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let defend_id = hand_card_id(&state, DEFEND_R_ID);

        let next = apply_combat_action(&state, defend_action(&state)).expect("Defend applies");

        assert!(!next.piles.hand.iter().any(|card| card.id == defend_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == defend_id));
    }

    #[test]
    fn bash_decreases_monster_hp_by_eight() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 8);
    }

    #[test]
    fn bash_adds_two_vulnerable_to_monster() {
        let state = CombatState::initial_fixture();

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.monsters[0].powers.vulnerable, 2);
    }

    #[test]
    fn bash_decreases_energy_by_two_and_moves_to_discard() {
        let state = CombatState::initial_fixture();
        let bash_id = hand_card_id(&state, BASH_ID);

        let next = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");

        assert_eq!(next.player.energy, state.player.energy - 2);
        assert!(!next.piles.hand.iter().any(|card| card.id == bash_id));
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == bash_id));
    }

    #[test]
    fn bash_is_illegal_with_less_than_two_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 1;

        assert_eq!(
            apply_combat_action(&state, bash_action(&state)),
            Err(SimError::IllegalAction("card is unaffordable"))
        );
    }

    #[test]
    fn strike_event_log_records_ordered_internal_actions() {
        let state = CombatState::initial_fixture();
        let strike_id = hand_strike_id(&state);

        let transition =
            apply_combat_action_with_events(&state, strike_action(&state)).expect("Strike applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: strike_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::DealDamage {
                    target: MonsterId::new(1),
                    amount: 6,
                },
                InternalAction::MoveCard {
                    card_id: strike_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    #[test]
    fn defend_event_log_records_gain_block() {
        let state = CombatState::initial_fixture();
        let defend_id = hand_card_id(&state, DEFEND_R_ID);

        let transition =
            apply_combat_action_with_events(&state, defend_action(&state)).expect("Defend applies");

        assert_eq!(
            transition.event_log,
            vec![
                InternalAction::PlayCard { card_id: defend_id },
                InternalAction::SpendEnergy { amount: 1 },
                InternalAction::GainBlock { amount: 5 },
                InternalAction::MoveCard {
                    card_id: defend_id,
                    from: CardPile::Hand,
                    to: CardPile::DiscardPile,
                },
            ]
        );
    }

    fn strike_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_strike_id(state),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_strike_id(state: &CombatState) -> CardId {
        hand_card_id(state, STRIKE_R_ID)
    }

    fn defend_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, DEFEND_R_ID),
            target: None,
        }
    }

    fn bash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BASH_ID),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_card_id(state: &CombatState, content_id: crate::ContentId) -> CardId {
        state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == content_id)
            .expect("card is in hand")
            .id
    }
}
