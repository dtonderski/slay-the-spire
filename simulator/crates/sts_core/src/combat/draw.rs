use crate::{
    combat::CombatState,
    rng::{RngStream, SimulatorRng, StsRng},
};

pub fn draw_cards(state: &mut CombatState, count: usize, rng: &mut SimulatorRng) {
    for _ in 0..count {
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw(state, rng);
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        state.piles.hand.push(state.piles.draw_pile.remove(0));
    }
}

pub fn draw_cards_with_sts_rng(state: &mut CombatState, count: usize, rng: &mut StsRng) {
    for _ in 0..count {
        if state.piles.draw_pile.is_empty() {
            shuffle_discard_into_draw_sts(state, rng);
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        state.piles.hand.push(state.piles.draw_pile.remove(0));
    }
}

fn shuffle_discard_into_draw(state: &mut CombatState, rng: &mut SimulatorRng) {
    if state.piles.discard_pile.is_empty() {
        return;
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);

    for index in (1..state.piles.draw_pile.len()).rev() {
        let swap_with = rng.next_usize(RngStream::Shuffle, "combat::draw::shuffle", index + 1);
        state.piles.draw_pile.swap(index, swap_with);
    }
}

fn shuffle_discard_into_draw_sts(state: &mut CombatState, rng: &mut StsRng) {
    if state.piles.discard_pile.is_empty() {
        return;
    }

    state.piles.draw_pile.append(&mut state.piles.discard_pile);
    rng.collections_shuffle(&mut state.piles.draw_pile);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{legal_combat_actions, CardInstance, ContentId};

    #[test]
    fn draw_order_is_deterministic_without_shuffle() {
        let mut first = fixture_with_draw_pile();
        let mut second = fixture_with_draw_pile();
        let mut first_rng = SimulatorRng::new(1);
        let mut second_rng = SimulatorRng::new(1);

        draw_cards(&mut first, 2, &mut first_rng);
        draw_cards(&mut second, 2, &mut second_rng);

        assert_eq!(first.piles.hand, second.piles.hand);
        assert_eq!(first.piles.hand[0].id, crate::CardId::new(10));
        assert_eq!(first.piles.hand[1].id, crate::CardId::new(11));
        assert!(first_rng.log().is_empty());
    }

    #[test]
    fn shuffle_consumes_logged_placeholder_rng() {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(crate::CardId::new(20), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(21), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(22), ContentId::new(1)),
        ];
        let mut rng = SimulatorRng::new(3);

        draw_cards(&mut state, 2, &mut rng);

        assert_eq!(state.piles.hand.len(), 2);
        assert_eq!(rng.log().len(), 2);
        assert!(rng
            .log()
            .iter()
            .all(|draw| draw.stream == RngStream::Shuffle));
    }

    #[test]
    fn placeholder_shuffle_is_deterministic_but_not_claimed_game_compatible() {
        let mut first = fixture_with_discard_only();
        let mut second = fixture_with_discard_only();
        let mut first_rng = SimulatorRng::new(99);
        let mut second_rng = SimulatorRng::new(99);

        draw_cards(&mut first, 3, &mut first_rng);
        draw_cards(&mut second, 3, &mut second_rng);

        assert_eq!(first.piles.hand, second.piles.hand);
        assert_eq!(first_rng.log(), second_rng.log());
    }

    #[test]
    fn legal_actions_and_serialization_consume_no_rng() {
        let state = CombatState::initial_fixture();
        let rng = SimulatorRng::new(5);
        let before_log_len = rng.log().len();

        let _actions = legal_combat_actions(&state);
        let _json = serde_json::to_string(&state).expect("state serializes");

        assert_eq!(rng.log().len(), before_log_len);
    }

    fn fixture_with_draw_pile() -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile = vec![
            CardInstance::new(crate::CardId::new(10), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(11), ContentId::new(1)),
        ];
        state
    }

    fn fixture_with_discard_only() -> CombatState {
        let mut state = CombatState::initial_fixture();
        state.piles.hand.clear();
        state.piles.draw_pile.clear();
        state.piles.discard_pile = vec![
            CardInstance::new(crate::CardId::new(30), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(31), ContentId::new(1)),
            CardInstance::new(crate::CardId::new(32), ContentId::new(1)),
        ];
        state
    }
}
