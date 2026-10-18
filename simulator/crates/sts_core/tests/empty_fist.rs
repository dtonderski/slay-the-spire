//! Source: PC EmptyFist.use/upgrade, ChangeStanceAction, DamageAction cleanup.
//! Synthetic controls supplement the natural FIDL01336 step1184 boundary.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{EMPTY_FIST_ANY_COLOR_ID, FLURRY_OF_BLOWS_ANY_COLOR_ID},
        shop_pool::shop_card_content_id,
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState,
};
fn fixture(upgraded: bool) -> CombatState {
    let mut state = CombatState::cultist_fixture();
    state.piles.hand.clear();
    state.piles.discard_pile.clear();
    let mut card = CardInstance::new(CardId::new(99), EMPTY_FIST_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    state.piles.hand.push(card);
    state.player.energy = 3;
    state.validate().unwrap();
    state
}
fn play(state: &CombatState) -> CombatState {
    apply_combat_action(
        state,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: Some(state.monsters[0].id),
        },
    )
    .unwrap()
}
#[test]
fn base_and_upgrade_attack_use_old_stance_then_exit_to_neutral() {
    assert_eq!(shop_card_content_id("EMPTY_FIST"), EMPTY_FIST_ANY_COLOR_ID);
    for upgraded in [false, true] {
        for stance in 0..4 {
            let mut state = fixture(upgraded);
            state.player.powers.calm = i32::from(stance == 1);
            state.player.powers.wrath = i32::from(stance == 2);
            state.player.powers.divinity = i32::from(stance == 3);
            let next = play(&state);
            let base = if upgraded { 14 } else { 9 };
            let multiplier = match stance {
                2 => 2,
                3 => 3,
                _ => 1,
            };
            assert_eq!(
                next.monsters[0].hp,
                state.monsters[0].hp - base * multiplier
            );
            assert_eq!(next.player.energy, if stance == 1 { 4 } else { 2 });
            assert_eq!(
                next.player.powers.calm + next.player.powers.wrath + next.player.powers.divinity,
                0
            );
            assert_eq!(next.piles.discard_pile.last().unwrap().id, CardId::new(99));
            next.validate().unwrap();
        }
    }
}
#[test]
fn copied_attack_recalculates_after_original_stance_exit() {
    let mut state = fixture(false);
    state.player.powers.wrath = 1;
    state.duplication_potion_pending = true;
    state.duplication_potion_stacks = 1;
    let next = play(&state);
    assert_eq!(next.monsters[0].hp, state.monsters[0].hp - 18 - 9);
    assert_eq!(next.player.energy, 2);
    assert_eq!(next.piles.discard_pile.len(), 1);
}
#[test]
fn real_stance_change_returns_flurry_but_neutral_noop_does_not() {
    for calm in [false, true] {
        let mut state = fixture(false);
        state.player.powers.calm = i32::from(calm);
        state.piles.discard_pile.push(CardInstance::new(
            CardId::new(77),
            FLURRY_OF_BLOWS_ANY_COLOR_ID,
        ));
        let next = play(&state);
        assert_eq!(
            next.piles.hand.iter().any(|c| c.id == CardId::new(77)),
            calm
        );
    }
}
#[test]
fn final_lethal_clears_queued_change_stance_without_calm_exit_energy() {
    let mut state = fixture(false);
    state.player.powers.calm = 1;
    state.monsters[0].hp = 1;
    let next = play(&state);
    assert!(!next.monsters[0].alive);
    assert_eq!(next.player.powers.calm, 1);
    assert_eq!(next.player.energy, 2);
}
#[test]
fn legal_actions_and_snapshot_restore_are_deterministic() {
    let state = fixture(false);
    let before = serde_json::to_value(&state).unwrap();
    assert!(legal_combat_actions(&state)
        .unwrap()
        .contains(&CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: Some(state.monsters[0].id)
        }));
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    let restored: CombatState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(play(&state)).unwrap(),
        serde_json::to_value(play(&restored)).unwrap()
    );
}
