//! Source-backed rules, not real-game trace parity evidence.
//! PC desktop StrengthPower::{stackPower, reducePower} clamps visible Strength
//! to [-999, 999]. Flex::use separately applies LoseStrengthPower with the full
//! nominal amount; its inherited AbstractPower::stackPower does not cap stacks.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::cards::{FLEX_ID, FLEX_PLUS_ID, PANACEA_ID, STRIKE_R_ID},
    CardId, CardInstance, CombatAction, CombatState, ContentId,
};

fn setup(permanent: i32, temporary: i32, artifact: i32, cards: &[ContentId]) -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.player.powers.strength = permanent;
    state.player.temp_strength = temporary;
    state.player.powers.artifact = artifact;
    state.piles.hand = cards
        .iter()
        .enumerate()
        .map(|(i, content)| CardInstance::new(CardId::new(i as u64 + 1), *content))
        .collect();
    state.validate().unwrap();
    state
}

fn play(state: &CombatState, index: u64) -> CombatState {
    let action = CombatAction::PlayCard {
        card_id: CardId::new(index),
        target: None,
    };
    let before = serde_json::to_value(state).unwrap();
    let restored: CombatState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_combat_action(state, action).unwrap();
    let repeated = apply_combat_action(&restored, action).unwrap();
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
    assert_eq!(serde_json::to_value(state).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next.rng).unwrap(),
        serde_json::to_value(&state.rng).unwrap()
    );
    next.validate().unwrap();
    println!(
        "temporary_strength_transition={}",
        serde_json::json!({
            "initial": before, "play_card_id": index,
            "visible_strength": next.player.powers.strength + next.player.temp_strength,
            "permanent_component": next.player.powers.strength,
            "pending_loss": next.player.temp_strength,
            "artifact": next.player.powers.artifact,
        })
    );
    next
}

#[test]
fn flex_and_upgrade_cap_visible_strength_and_expire_the_full_nominal_amount() {
    for (card, amount) in [(FLEX_ID, 2), (FLEX_PLUS_ID, 4)] {
        for permanent in [998, 999] {
            let next = play(&setup(permanent, 0, 0, &[card]), 1);
            assert_eq!(next.player.powers.strength + next.player.temp_strength, 999);
            assert_eq!(next.player.temp_strength, amount);
            assert_eq!(next.player.powers.strength, 999 - amount);
            let expired = apply_combat_action(&next, CombatAction::EndTurn).unwrap();
            assert_eq!(expired.player.temp_strength, 0);
            assert_eq!(expired.player.powers.strength, 999 - amount);
        }
    }
}

#[test]
fn artifact_blocked_flex_is_permanent_but_still_caps_the_combined_power() {
    for card in [FLEX_ID, FLEX_PLUS_ID] {
        let next = play(&setup(997, 2, 1, &[card]), 1);
        assert_eq!(next.player.powers.strength + next.player.temp_strength, 999);
        assert_eq!(next.player.temp_strength, 2);
        assert_eq!(next.player.powers.strength, 997);
        assert_eq!(next.player.powers.artifact, 0);
        let expired = apply_combat_action(&next, CombatAction::EndTurn).unwrap();
        assert_eq!(expired.player.powers.strength, 997);
        assert_eq!(expired.player.temp_strength, 0);
    }
}

#[test]
fn multiple_flexes_keep_uncapped_loss_stacks_while_capping_visible_strength() {
    // LoseStrengthPower inherits uncapped AbstractPower stacking: do not cap
    // this pending loss to 999 or to the actual gain admitted by StrengthPower.
    let first = play(&setup(-1, 1000, 0, &[FLEX_ID, FLEX_PLUS_ID]), 1);
    assert_eq!(
        first.player.powers.strength + first.player.temp_strength,
        999
    );
    assert_eq!(first.player.temp_strength, 1002);
    let second = play(&first, 2);
    assert_eq!(
        second.player.powers.strength + second.player.temp_strength,
        999
    );
    assert_eq!(second.player.temp_strength, 1006);
    let expired = apply_combat_action(&second, CombatAction::EndTurn).unwrap();
    assert_eq!(expired.player.powers.strength, -7);
    assert_eq!(expired.player.temp_strength, 0);
}

#[test]
fn artifact_gained_after_flex_blocks_expiry_without_restoring_truncated_strength() {
    let flexed = play(&setup(999, 0, 0, &[FLEX_ID, PANACEA_ID]), 1);
    let protected = play(&flexed, 2);
    assert_eq!(
        protected.player.powers.strength + protected.player.temp_strength,
        999
    );
    assert_eq!(protected.player.powers.artifact, 1);
    let expired = apply_combat_action(&protected, CombatAction::EndTurn).unwrap();
    assert_eq!(expired.player.powers.strength, 999);
    assert_eq!(expired.player.temp_strength, 0);
    assert_eq!(expired.player.powers.artifact, 0);
}

#[test]
fn clipped_flex_strength_drives_attack_damage_not_only_display() {
    // An explicit synthetic initial HP lets the Strike damage remain observable.
    let mut initial = setup(999, 0, 0, &[FLEX_ID, STRIKE_R_ID]);
    initial.monsters[0].hp = 3000;
    initial.monsters[0].max_hp = 3000;
    initial.validate().unwrap();
    let flexed = play(&initial, 1);
    let attack = CombatAction::PlayCard {
        card_id: CardId::new(2),
        target: Some(flexed.monsters[0].id),
    };
    let next = apply_combat_action(&flexed, attack).unwrap();
    let restored: CombatState =
        serde_json::from_value(serde_json::to_value(&flexed).unwrap()).unwrap();
    let repeated = apply_combat_action(&restored, attack).unwrap();
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
    assert_eq!(next.monsters[0].hp, 3000 - (6 + 999));
    next.validate().unwrap();
}

#[test]
fn expiry_clamps_negative_strength_without_capping_the_pending_loss() {
    let next = play(&setup(-1998, 999, 0, &[FLEX_PLUS_ID]), 1);
    assert_eq!(
        next.player.powers.strength + next.player.temp_strength,
        -995
    );
    assert_eq!(next.player.temp_strength, 1003);
    let expired = apply_combat_action(&next, CombatAction::EndTurn).unwrap();
    assert_eq!(expired.player.powers.strength, -999);
    assert_eq!(expired.player.temp_strength, 0);
}

#[test]
fn ordinary_and_negative_strength_flex_behavior_is_unchanged() {
    for permanent in [-999, -5, 0, 5, 990] {
        for artifact in [0, 1] {
            let next = play(&setup(permanent, 0, artifact, &[FLEX_ID]), 1);
            assert_eq!(
                next.player.powers.strength + next.player.temp_strength,
                permanent + 2
            );
            assert_eq!(next.player.temp_strength, if artifact == 0 { 2 } else { 0 });
            assert_eq!(next.player.powers.artifact, 0);
            let expired = apply_combat_action(&next, CombatAction::EndTurn).unwrap();
            assert_eq!(
                expired.player.powers.strength,
                permanent + if artifact == 0 { 0 } else { 2 }
            );
            assert_eq!(expired.player.temp_strength, 0);
        }
    }
}
