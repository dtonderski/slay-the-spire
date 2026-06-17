use crate::{
    combat::{CombatPhase, CombatState},
    content::monsters::FIXED_SIMPLE_MONSTER,
};

const HAND_SIZE: usize = 5;

pub fn end_player_turn(state: &CombatState) -> CombatState {
    let mut next = state.clone();

    discard_hand(&mut next);
    run_fixed_monster_turn(&mut next);

    if next.player.hp <= 0 {
        next.phase = CombatPhase::Lost;
        return next;
    }

    draw_next_hand_without_shuffle(&mut next);
    next.phase = CombatPhase::WaitingForPlayer;
    next
}

fn discard_hand(state: &mut CombatState) {
    state.piles.discard_pile.append(&mut state.piles.hand);
}

fn run_fixed_monster_turn(state: &mut CombatState) {
    if state.monsters.iter().any(|monster| monster.alive) {
        deal_damage_to_player(state, FIXED_SIMPLE_MONSTER.attack_damage);
    }
    state.player.block = 0;
}

fn deal_damage_to_player(state: &mut CombatState, amount: i32) {
    let blocked = state.player.block.min(amount);
    state.player.block -= blocked;
    state.player.hp -= amount - blocked;
}

fn draw_next_hand_without_shuffle(state: &mut CombatState) {
    while state.piles.hand.len() < HAND_SIZE && !state.piles.draw_pile.is_empty() {
        state.piles.hand.push(state.piles.draw_pile.remove(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::cards::STRIKE_R_ID, CombatAction};

    #[test]
    fn end_turn_is_legal() {
        let state = CombatState::initial_fixture();

        assert!(crate::legal_combat_actions(&state).contains(&CombatAction::EndTurn));
    }

    #[test]
    fn end_turn_moves_remaining_hand_to_discard() {
        let state = CombatState::initial_fixture();
        let starting_hand_ids: Vec<_> = state.piles.hand.iter().map(|card| card.id).collect();

        let next = end_player_turn(&state);

        for card_id in starting_hand_ids {
            assert!(next
                .piles
                .discard_pile
                .iter()
                .any(|card| card.id == card_id));
        }
    }

    #[test]
    fn monster_attack_reduces_block_before_hp() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 4;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 18);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn block_clears_after_simplified_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 10;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 0);
    }

    #[test]
    fn next_hand_is_drawn_deterministically_without_shuffle() {
        let state = CombatState::initial_fixture();

        let next = end_player_turn(&state);

        assert_eq!(next.piles.hand.len(), 1);
        assert_eq!(next.piles.hand[0].content_id, STRIKE_R_ID);
        assert!(next.piles.draw_pile.is_empty());
    }

    #[test]
    fn combat_can_reach_lost_state() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 6;
        state.player.block = 0;

        let next = end_player_turn(&state);

        assert_eq!(next.phase, CombatPhase::Lost);
    }
}
