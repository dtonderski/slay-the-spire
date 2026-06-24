use crate::{
    combat::turn_powers::{apply_end_of_monster_turn_powers, apply_end_of_player_turn_powers},
    combat::{draw::apply_snecko_eye_cost_randomization, hand::resolve_end_of_turn_hand},
    combat::{CombatPhase, CombatState},
    content::monsters::{
        apply_monster_intent, clear_lagavulin_metallicize_if_awake, prepare_monster_intent,
    },
    rng::JavaRng,
};

const HAND_SIZE: usize = 5;

/// Simplified milestone timing:
///
/// 1. Ending the player turn discards the remaining hand.
/// 2. The monster turn consumes current player block before HP.
/// 3. Player block clears after the monster turn, before the next hand is drawn.
/// 4. Monster vulnerable decrements by 1 during monster-turn cleanup.
/// 5. The next player turn refills energy and draws from the draw pile without shuffle.
pub fn end_player_turn(state: &CombatState) -> CombatState {
    let mut next = state.clone();

    resolve_end_of_turn_hand(&mut next);
    apply_end_of_player_turn_powers(&mut next);
    crate::relic::apply_end_of_player_turn_relics(&mut next);
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
    crate::relic::reset_turn_relic_counters(state);
    reset_turn_only_temp_costs(state);
    if !crate::relic::preserves_energy_between_turns(&state.relics) {
        state.player.energy = state.player.max_energy;
    }
    state.player.cannot_draw = false;
    state.player.temp_strength = 0;
    if state.player.temp_dexterity > 0 {
        state.player.powers.dexterity -= state.player.temp_dexterity;
        state.player.temp_dexterity = 0;
    }
    crate::relic::apply_start_of_player_turn_relics(state);
    draw_next_hand_without_shuffle(state);
    prepare_next_intents(state);
    state.phase = CombatPhase::WaitingForPlayer;
}

fn reset_turn_only_temp_costs(state: &mut CombatState) {
    for pile in [
        &mut state.piles.hand,
        &mut state.piles.draw_pile,
        &mut state.piles.discard_pile,
        &mut state.piles.exhaust_pile,
    ] {
        for card in pile {
            if card.temp_cost_turn_only {
                card.temp_cost = None;
                card.temp_cost_turn_only = false;
            }
        }
    }
}

fn run_monster_turn(state: &mut CombatState) {
    let ascension = state.ascension;
    let relics = state.relics.clone();
    let CombatState {
        player,
        monsters,
        piles,
        phase: _,
        ..
    } = state;
    let mut pending_damage = Vec::new();
    for monster in monsters.iter_mut().filter(|monster| monster.alive) {
        clear_lagavulin_metallicize_if_awake(monster);
        let player_snapshot = player.clone();
        let damage =
            apply_monster_intent(monster, player, piles, ascension, &player_snapshot, &relics);
        if damage > 0 {
            pending_damage.push(damage);
        }
    }

    for damage in pending_damage {
        deal_damage_to_player(state, damage);
    }

    for monster in &mut state.monsters {
        if monster.alive {
            if monster.powers.vulnerable > 0 {
                monster.powers.vulnerable -= 1;
            }
            if monster.powers.weak > 0 {
                monster.powers.weak -= 1;
            }
            apply_end_of_monster_turn_powers(monster);
        }
    }

    if state.player.powers.vulnerable > 0 {
        state.player.powers.vulnerable -= 1;
    }
    if state.player.powers.intangible > 0 {
        state.player.powers.intangible -= 1;
    }

    apply_turn_transition_block_loss(state);
}

fn apply_turn_transition_block_loss(state: &mut CombatState) {
    if state.relics.contains(&crate::Relic::Calipers) {
        state.player.block = (state.player.block - crate::relic::CALIPERS_BLOCK_LOSS).max(0);
    } else {
        state.player.block = 0;
    }
}

fn deal_damage_to_player(state: &mut CombatState, amount: i32) {
    let incoming = if state.player.powers.intangible > 0 && amount > 1 {
        1
    } else {
        amount
    };
    let blocked = state.player.block.min(incoming);
    state.player.block -= blocked;
    let mitigated =
        crate::relic::mitigate_unblocked_attack_damage(&state.relics, incoming - blocked);
    let hp_damage = crate::relic::apply_buffer_to_hp_loss(&mut state.player.powers, mitigated);
    state.player.hp -= hp_damage;
    crate::relic::apply_player_hp_loss_relics(state, hp_damage);
    if hp_damage > 0 && state.player.powers.plated_armor > 0 {
        state.player.powers.plated_armor -= 1;
    }
}

fn draw_next_hand_without_shuffle(state: &mut CombatState) {
    while state.piles.hand.len() < target_hand_size(state) {
        if state.piles.draw_pile.is_empty() {
            if let Some(rng) = state.shuffle_rng.as_mut() {
                if !state.piles.discard_pile.is_empty() {
                    state.piles.draw_pile.append(&mut state.piles.discard_pile);
                    let shuffle_seed = rng.random_long();
                    JavaRng::new(shuffle_seed).collections_shuffle(&mut state.piles.draw_pile);
                    crate::relic::apply_shuffle_relics(state);
                }
            }
        }

        if state.piles.draw_pile.is_empty() {
            break;
        }

        if let Some(mut card) = state.piles.draw_pile.pop() {
            apply_snecko_eye_cost_randomization(state, &mut card);
            state.piles.hand.push(card);
        }
    }
}

fn target_hand_size(state: &CombatState) -> usize {
    HAND_SIZE
        + if state.relics.contains(&crate::Relic::SneckoEye) {
            crate::relic::SNECKO_EYE_DRAW
        } else {
            0
        }
}

fn prepare_next_intents(state: &mut CombatState) {
    for monster in &mut state.monsters {
        if monster.alive {
            monster.intent = prepare_monster_intent(monster);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_combat_action,
        combat::MonsterIntent,
        content::cards::{BASH_ID, STRIKE_R_ID},
        content::monsters::FIXED_SIMPLE_MONSTER,
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
    fn plated_armor_blocks_then_loses_stack_on_unblocked_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.plated_armor = 4;
        state.player.hp = 20;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 18);
        assert_eq!(next.player.powers.plated_armor, 3);
    }

    #[test]
    fn plated_armor_does_not_decrement_when_attack_is_fully_blocked() {
        let mut state = CombatState::initial_fixture();
        state.player.powers.plated_armor = 4;
        state.player.block = 10;
        state.player.hp = 20;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.powers.plated_armor, 4);
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
    fn torii_reduces_small_unblocked_monster_attack_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 1;
        state.relics = vec![crate::Relic::Torii];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
    }

    #[test]
    fn tungsten_rod_reduces_unblocked_monster_attack_damage() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 4;
        state.relics = vec![crate::Relic::TungstenRod];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
    }

    #[test]
    fn buffer_prevents_next_monster_attack_hp_loss_after_block() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 4;
        state.player.powers.buffer = 1;

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.powers.buffer, 0);
    }

    #[test]
    fn intangible_caps_monster_attack_damage_until_monster_turn_cleanup() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.powers.intangible = 1;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 19);
        assert_eq!(next.player.powers.intangible, 0);
    }

    #[test]
    fn self_forming_clay_block_can_absorb_later_monster_attack() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 0;
        state.relics = vec![crate::Relic::SelfFormingClay];
        state.monsters[0].intent = MonsterIntent::Attack { damage: 2 };
        let mut second_monster = state.monsters[0].clone();
        second_monster.id = crate::MonsterId::new(2);
        second_monster.intent = MonsterIntent::Attack { damage: 2 };
        state.monsters.push(second_monster);

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
        assert_eq!(next.player.energy, crate::combat::state::BASE_PLAYER_ENERGY);
    }

    #[test]
    fn cannot_draw_clears_at_start_of_next_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.cannot_draw = true;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert!(!next.player.cannot_draw);
    }

    #[test]
    fn temp_strength_clears_at_start_of_next_player_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.temp_strength = 2;
        state.piles.draw_pile.clear();

        let next = end_player_turn(&state);

        assert_eq!(next.player.temp_strength, 0);
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
    fn calipers_loses_fifteen_block_instead_of_all_block_after_monster_turn() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 30;
        state.relics = vec![crate::Relic::Calipers];

        let next = end_player_turn(&state);

        assert_eq!(next.player.hp, 20);
        assert_eq!(next.player.block, 9);
    }

    #[test]
    fn calipers_floors_retained_block_at_zero() {
        let mut state = CombatState::initial_fixture();
        state.player.hp = 20;
        state.player.block = 16;
        state.relics = vec![crate::Relic::Calipers];

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
    fn snecko_eye_draws_seven_each_turn_and_randomizes_drawn_costs() {
        let mut state = CombatState::initial_fixture();
        state.relics = vec![crate::Relic::SneckoEye];
        state.card_random_rng = Some(crate::rng::StsRng::new(123));
        state.piles.hand.clear();
        state.piles.draw_pile = (10..20)
            .map(|id| crate::CardInstance::new(CardId::new(id), STRIKE_R_ID))
            .collect();

        start_player_turn(&mut state);

        assert_eq!(state.piles.hand.len(), 7);
        assert!(state.piles.hand.iter().all(|card| card.temp_cost.is_some()));
        assert_eq!(
            state.card_random_rng.as_ref().expect("card rng").counter(),
            7
        );
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
