use crate::{
    combat::hand::resolve_end_of_turn_hand,
    combat::turn_powers::{
        apply_end_of_monster_turn_powers, apply_end_of_player_turn_powers, monster_attack_damage,
    },
    combat::{CombatPhase, CombatState, MonsterIntent},
    content::monsters::FIXED_SIMPLE_MONSTER,
};

const HAND_SIZE: usize = 5;
const PLAYER_TURN_ENERGY: i32 = 3;

/// Simplified milestone timing:
///
/// 1. Ending the player turn discards the remaining hand.
/// 2. The monster turn consumes current player block before HP.
/// 3. Player block clears after the monster turn, before the next hand is drawn.
/// 4. Monster vulnerable decrements by 1 at end of monster turn.
/// 5. The next player turn refills energy and draws from the draw pile without shuffle.
pub fn end_player_turn(state: &CombatState) -> CombatState {
    let mut next = state.clone();

    resolve_end_of_turn_hand(&mut next);
    apply_end_of_player_turn_powers(&mut next);
    next.phase = CombatPhase::MonsterTurn;
    run_monster_turn(&mut next);

    if next.player.hp <= 0 {
        next.phase = CombatPhase::Lost;
        return next;
    }

    start_player_turn(&mut next);
    next
}

pub fn start_player_turn(state: &mut CombatState) {
    state.player.energy = PLAYER_TURN_ENERGY;
    draw_next_hand_without_shuffle(state);
    prepare_next_intents(state);
    state.phase = CombatPhase::WaitingForPlayer;
}

fn run_monster_turn(state: &mut CombatState) {
    let total_damage: i32 = state
        .monsters
        .iter()
        .filter(|monster| monster.alive)
        .map(|monster| match monster.intent {
            MonsterIntent::Attack { damage } => monster_attack_damage(monster, damage),
        })
        .sum();

    if total_damage > 0 {
        deal_damage_to_player(state, total_damage);
    }

    for monster in &mut state.monsters {
        if monster.alive {
            if monster.powers.vulnerable > 0 {
                monster.powers.vulnerable -= 1;
            }
            apply_end_of_monster_turn_powers(monster);
        }
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

fn prepare_next_intents(state: &mut CombatState) {
    for monster in &mut state.monsters {
        if monster.alive {
            monster.intent = MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_combat_action,
        content::cards::{BASH_ID, STRIKE_R_ID},
        ids::CardId,
        CombatAction,
    };

    #[test]
    fn metallicize_grants_block_before_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.metallicize = 4;
        state.player.hp = 30;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 28);
    }

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
    fn end_turn_enters_next_player_turn_with_refilled_energy() {
        let mut state = CombatState::initial_fixture();
        state.player.energy = 0;

        let next = end_player_turn(&state);

        assert_eq!(next.phase, CombatPhase::WaitingForPlayer);
        assert_eq!(next.player.energy, PLAYER_TURN_ENERGY);
    }

    #[test]
    fn next_intent_placeholder_is_fixed_attack() {
        let state = CombatState::initial_fixture();

        let next = end_player_turn(&state);

        assert_eq!(
            next.monsters[0].intent,
            MonsterIntent::Attack {
                damage: FIXED_SIMPLE_MONSTER.attack_damage,
            }
        );
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

    #[test]
    fn vulnerable_decrements_at_end_of_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.monsters[0].hp = 100;
        state = apply_combat_action(&state, bash_action(&state)).expect("Bash applies");
        assert_eq!(state.monsters[0].powers.vulnerable, 2);

        state = end_player_turn(&state);
        assert_eq!(state.monsters[0].powers.vulnerable, 1);

        state = end_player_turn(&state);
        assert_eq!(state.monsters[0].powers.vulnerable, 0);
    }

    fn bash_action(state: &CombatState) -> CombatAction {
        CombatAction::PlayCard {
            card_id: hand_card_id(state, BASH_ID),
            target: Some(state.monsters[0].id),
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
