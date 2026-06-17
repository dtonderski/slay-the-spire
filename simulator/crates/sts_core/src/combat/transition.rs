use crate::{
    action::CombatAction,
    combat::{damage::deal_unmodified_damage_to_monster, validate_combat_action, CombatPhase},
    content::cards::{get_card_definition, STRIKE_R_ID},
    ids::{CardId, MonsterId},
    CardInstance, CombatState, SimError, SimResult,
};

pub fn apply_combat_action(state: &CombatState, action: CombatAction) -> SimResult<CombatState> {
    validate_combat_action(state, action)?;

    match action {
        CombatAction::PlayCard { card_id, target } => apply_play_card(state, card_id, target),
        CombatAction::EndTurn => Err(SimError::IllegalAction(
            "EndTurn transition is not implemented",
        )),
    }
}

fn apply_play_card(
    state: &CombatState,
    card_id: CardId,
    target: Option<MonsterId>,
) -> SimResult<CombatState> {
    let card = find_hand_card(state, card_id)?;
    let definition =
        get_card_definition(card.content_id).ok_or(SimError::UnknownContent(card.content_id))?;

    if definition.id != STRIKE_R_ID {
        return Err(SimError::IllegalAction(
            "card transition is not implemented",
        ));
    }

    apply_strike(
        state,
        card_id,
        target.expect("validated Strike has a target"),
    )
}

fn apply_strike(state: &CombatState, card_id: CardId, target: MonsterId) -> SimResult<CombatState> {
    let mut next = state.clone();
    let card = remove_card_from_hand(&mut next, card_id)?;

    next.player.energy -= 1;

    let monster = next
        .monsters
        .iter_mut()
        .find(|monster| monster.id == target && monster.alive)
        .ok_or(SimError::IllegalAction("target is not a living monster"))?;

    deal_unmodified_damage_to_monster(monster, 6);
    next.piles.discard_pile.push(card);

    if next.monsters.iter().all(|monster| !monster.alive) {
        next.phase = CombatPhase::Won;
    } else {
        next.phase = CombatPhase::WaitingForPlayer;
    }

    Ok(next)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::STRIKE_R_ID;

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

    fn strike_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_strike_id(state),
            target: Some(MonsterId::new(1)),
        }
    }

    fn hand_strike_id(state: &CombatState) -> CardId {
        state
            .piles
            .hand
            .iter()
            .find(|card| card.content_id == STRIKE_R_ID)
            .expect("Strike is in hand")
            .id
    }
}
