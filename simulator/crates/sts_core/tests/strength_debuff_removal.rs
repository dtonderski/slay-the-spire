//! Source-backed synthetic regressions, not dedicated real-game trace parity.
//! OrangePellets.onUseCard queues RemoveDebuffsAction.update, which classifies
//! the actual StrengthPower (StrengthPower.updateDescription), not our split
//! permanent component. Removing LoseStrengthPower does not execute its expiry.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::cards::{DEFEND_R_ID, FLEX_ID, INFLAME_ID, STRIKE_R_ID},
    CardId, CardInstance, CombatAction, CombatState, ContentId, Relic,
};

fn setup(permanent: i32, loss: i32, skill: ContentId) -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.player.powers.strength = permanent;
    state.player.temp_strength = loss;
    state.player.relics.push(Relic::OrangePellets);
    state.monsters[0].hp = 4000;
    state.monsters[0].max_hp = 4000;
    state.piles.hand = [skill, INFLAME_ID, STRIKE_R_ID]
        .into_iter()
        .enumerate()
        .map(|(i, card)| CardInstance::new(CardId::new(i as u64 + 1), card))
        .collect();
    state.validate().unwrap();
    state
}

fn trigger(mut state: CombatState) -> CombatState {
    for index in 1..=3 {
        let action = CombatAction::PlayCard {
            card_id: CardId::new(index),
            target: if index == 3 {
                Some(state.monsters[0].id)
            } else {
                None
            },
        };
        let before = serde_json::to_value(&state).unwrap();
        let restored: CombatState = serde_json::from_value(before.clone()).unwrap();
        let next = apply_combat_action(&state, action).unwrap();
        let repeated = apply_combat_action(&restored, action).unwrap();
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
        assert_eq!(
            serde_json::to_value(&next).unwrap(),
            serde_json::to_value(repeated).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&next.rng).unwrap(),
            serde_json::to_value(&state.rng).unwrap()
        );
        next.validate().unwrap();
        println!(
            "strength_cleanse_transition={}",
            serde_json::json!({
                "initial": before, "action": action, "result": next,
            })
        );
        state = next;
    }
    state
}

fn assert_cleansed(state: &CombatState, strength: i32) {
    assert_eq!(state.player.temp_strength, 0);
    assert_eq!(state.player.powers.strength, strength);
    let expired = apply_combat_action(state, CombatAction::EndTurn).unwrap();
    assert_eq!(expired.player.temp_strength, 0);
    assert_eq!(expired.player.powers.strength, strength);
}

#[test]
fn orange_pellets_removes_negative_current_strength_without_inventing_a_buff() {
    // -5 + Flex(2) + Inflame(2) = -1: Strength and the pending loss are debuffs.
    assert_cleansed(&trigger(setup(-5, 0, FLEX_ID)), 0);
}

#[test]
fn orange_pellets_keeps_positive_current_strength_despite_negative_component() {
    // -3 + Flex(2) + Inflame(2) = 1: Strength is a buff, only the loss is removed.
    assert_cleansed(&trigger(setup(-3, 0, FLEX_ID)), 1);
}

#[test]
fn orange_pellets_does_not_recreate_strength_when_the_current_total_is_zero() {
    assert_cleansed(&trigger(setup(-4, 0, FLEX_ID)), 0);
}

#[test]
fn orange_pellets_preserves_bounded_strength_with_large_uncapped_pending_loss() {
    // Explicit valid initial powers: visible Strength 999, LoseStrength 1000.
    // Defend avoids relying on the separate, proposed Flex-bound fix (#97).
    assert_cleansed(&trigger(setup(-1, 1000, DEFEND_R_ID)), 999);
}

#[test]
fn subsequent_attack_uses_the_retained_current_strength() {
    let mut initial = setup(-3, 0, FLEX_ID);
    initial
        .piles
        .hand
        .push(CardInstance::new(CardId::new(5), STRIKE_R_ID));
    initial.validate().unwrap();
    let cleansed = trigger(initial);
    let attack = CombatAction::PlayCard {
        card_id: CardId::new(5),
        target: Some(cleansed.monsters[0].id),
    };
    let next = apply_combat_action(&cleansed, attack).unwrap();
    let restored: CombatState =
        serde_json::from_value(serde_json::to_value(&cleansed).unwrap()).unwrap();
    let repeated = apply_combat_action(&restored, attack).unwrap();
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
    assert_eq!(cleansed.monsters[0].hp - next.monsters[0].hp, 6 + 1);
    next.validate().unwrap();
}

#[test]
fn ordinary_positive_strength_is_retained_and_the_loss_no_longer_expires() {
    assert_cleansed(&trigger(setup(0, 0, FLEX_ID)), 4);
}

#[test]
fn no_loss_and_other_debuff_removal_preserve_existing_artifact() {
    let mut initial = setup(-4, 0, DEFEND_R_ID);
    initial.player.powers.weak = 1;
    initial.player.powers.frail = 1;
    initial.player.powers.vulnerable = 1;
    initial.player.powers.dexterity = -2;
    initial.player.temp_dexterity = 3;
    initial.player.powers.artifact = 2;
    initial.validate().unwrap();
    let next = trigger(initial);
    assert_cleansed(&next, 0);
    assert_eq!(next.player.powers.artifact, 2);
    assert_eq!(next.player.powers.weak, 0);
    assert_eq!(next.player.powers.frail, 0);
    assert_eq!(next.player.powers.vulnerable, 0);
    assert_eq!(next.player.powers.dexterity, 0);
    assert_eq!(next.player.temp_dexterity, 0);
}
