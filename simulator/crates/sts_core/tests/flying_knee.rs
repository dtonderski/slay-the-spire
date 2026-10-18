//! PC FlyingKnee.use / EnergizedPower.onEnergyRecharge, synthetic controls.
//! Natural regression: FIDL01535 step866; fixtures alone are not parity proof.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{DEFEND_R_ID, FLYING_KNEE_ANY_COLOR_ID},
        shop_pool::shop_card_content_id,
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState,
};
fn fixture(upgraded: bool) -> CombatState {
    let mut state = CombatState::cultist_fixture();
    state.piles.hand.clear();
    state.piles.discard_pile.clear();
    state.piles.draw_pile.clear();
    let mut card = CardInstance::new(CardId::new(99), FLYING_KNEE_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    state.piles.hand.push(card);
    for i in 0..10 {
        state
            .piles
            .draw_pile
            .push(CardInstance::new(CardId::new(200 + i), DEFEND_R_ID));
    }
    state.player.energy = 3;
    state.validate().unwrap();
    state
}
fn play(s: &CombatState) -> CombatState {
    apply_combat_action(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: Some(s.monsters[0].id),
        },
    )
    .unwrap()
}
#[test]
fn base_and_upgrade_damage_cost_and_next_turn_energy() {
    assert_eq!(
        shop_card_content_id("FLYING_KNEE"),
        FLYING_KNEE_ANY_COLOR_ID
    );
    for upgraded in [false, true] {
        let state = fixture(upgraded);
        let next = play(&state);
        assert_eq!(
            next.monsters[0].hp,
            state.monsters[0].hp - if upgraded { 11 } else { 8 }
        );
        assert_eq!(next.player.energy, 2);
        assert_eq!(next.player.energy_next_turn, 1);
        assert_eq!(next.piles.discard_pile.len(), 1);
        let next = apply_combat_action(&next, CombatAction::EndTurn).unwrap();
        assert_eq!(next.player.energy, 4);
        assert_eq!(next.player.energy_next_turn, 0);
        next.validate().unwrap();
    }
}
#[test]
fn no_draw_strength_pen_nib_and_positive_power_ignores_artifact() {
    let mut state = fixture(true);
    state.player.cannot_draw = true;
    state.player.powers.strength = 2;
    state.player.powers.artifact = 1;
    // Arming the relic through its internal simulator counter avoids relying on
    // an already-active scope that the new card transition intentionally resets.
    state
        .player
        .authority
        .relics
        .push(sts_core::adapter_internals::Relic::PenNib);
    state.pen_nib_attacks_played = 9;
    let before = serde_json::to_value(&state).unwrap();
    let actions = legal_combat_actions(&state).unwrap();
    assert!(actions.contains(&CombatAction::PlayCard {
        card_id: CardId::new(99),
        target: Some(state.monsters[0].id)
    }));
    assert_eq!(before, serde_json::to_value(&state).unwrap());
    let next = play(&state);
    assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 26);
    assert_eq!(next.player.energy_next_turn, 1);
    assert_eq!(next.player.powers.artifact, 1);
    assert_eq!(next.piles.draw_pile, state.piles.draw_pile);
    assert!(next.player.cannot_draw);
}
#[test]
fn duplicate_stacks_energized_without_paying_cost_twice() {
    let mut state = fixture(false);
    state.duplication_potion_pending = true;
    state.duplication_potion_stacks = 1;
    state.player.energy_next_turn = 3;
    let next = play(&state);
    assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 16);
    assert_eq!(next.player.energy, 2);
    assert_eq!(next.player.energy_next_turn, 5);
    assert_eq!(next.piles.discard_pile.len(), 1);
    next.validate().unwrap();
}
#[test]
fn lethal_damage_clears_later_power_application_not_existing_stacks() {
    let mut state = fixture(false);
    state.monsters[0].hp = 1;
    state.player.energy_next_turn = 2;
    let next = play(&state);
    assert!(!next.monsters[0].alive);
    assert_eq!(next.player.energy_next_turn, 2);
}
#[test]
fn energized_stacking_caps_at_native_999() {
    let mut state = fixture(false);
    state.player.energy_next_turn = 998;
    state.duplication_potion_pending = true;
    state.duplication_potion_stacks = 1;
    assert_eq!(play(&state).player.energy_next_turn, 999);
}
#[test]
fn energized_overflow_rejects_whole_transition() {
    let mut state = fixture(false);
    state.player.energy_next_turn = i32::MAX;
    let before = serde_json::to_value(&state).unwrap();
    assert!(apply_combat_action(
        &state,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: Some(state.monsters[0].id)
        }
    )
    .is_err());
    assert_eq!(before, serde_json::to_value(&state).unwrap());
}
