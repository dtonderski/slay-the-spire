//! Source-backed synthetic regressions, not dedicated real-game trace parity.
//! StrengthPotion.use applies StrengthPower (potency 2, doubled by Sacred Bark).
//! StrengthPower.stackPower bounds the actual current amount to +/-999.
use sts_core::adapter_internals::{
    apply_combat_action, apply_potion_action, CardId, CombatAction, Potion, Relic, RunAction,
    RunState, SimError,
};

fn setup(permanent: i32, pending_loss: i32, bark: bool) -> RunState {
    let mut run = RunState::combat_fixture_with_relics(if bark {
        vec![Relic::SacredBark]
    } else {
        Vec::new()
    });
    run.potions = vec![Potion::Strength];
    run.empty_potion_slots = vec![1, 2];
    let combat = run.combat.as_mut().unwrap();
    combat.player.powers.strength = permanent;
    combat.player.temp_strength = pending_loss;
    combat.monsters[0].hp = 4000;
    combat.monsters[0].max_hp = 4000;
    run.validate().unwrap();
    run
}

fn drink(initial: &RunState) -> RunState {
    let action = RunAction::UsePotion {
        slot: 0,
        target: None,
    };
    let before = serde_json::to_value(initial).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_potion_action(initial, action).unwrap();
    let repeated = apply_potion_action(&restored, action).unwrap();
    assert_eq!(serde_json::to_value(initial).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
    assert_eq!(
        next.combat.as_ref().unwrap().rng,
        initial.combat.as_ref().unwrap().rng
    );
    assert_eq!(next.potion_at_slot(0), None);
    next.validate().unwrap();
    println!(
        "strength_potion_transition={}",
        serde_json::json!({
            "initial": before, "action": action, "result": next,
        })
    );
    next
}

#[test]
fn strength_potion_caps_current_strength_with_and_without_sacred_bark() {
    for bark in [false, true] {
        for strength in [998, 999] {
            let next = drink(&setup(strength, 0, bark));
            assert_eq!(next.combat.as_ref().unwrap().player.powers.strength, 999);
        }
    }
}

#[test]
fn strength_potion_caps_the_combined_power_without_erasing_pending_loss() {
    for bark in [false, true] {
        let next = drink(&setup(996, 2, bark));
        let combat = next.combat.as_ref().unwrap();
        assert_eq!(
            combat.player.powers.strength + combat.player.temp_strength,
            999
        );
        assert_eq!(combat.player.powers.strength, 997);
        assert_eq!(combat.player.temp_strength, 2);
        let expired = apply_combat_action(combat, CombatAction::EndTurn).unwrap();
        assert_eq!(expired.player.powers.strength, 997);
        assert_eq!(expired.player.temp_strength, 0);
    }
}

#[test]
fn strength_potion_keeps_large_nominal_loss_and_negative_internal_component() {
    let next = drink(&setup(-1, 1000, false));
    let combat = next.combat.as_ref().unwrap();
    assert_eq!(
        combat.player.powers.strength + combat.player.temp_strength,
        999
    );
    assert_eq!(combat.player.powers.strength, -1);
    assert_eq!(combat.player.temp_strength, 1000);
    let expired = apply_combat_action(combat, CombatAction::EndTurn).unwrap();
    assert_eq!(expired.player.powers.strength, -1);
    assert_eq!(expired.player.temp_strength, 0);
}

#[test]
fn ordinary_and_negative_strength_potion_behavior_is_unchanged() {
    for strength in [-999, -5, 0, 5, 995] {
        for bark in [false, true] {
            let next = drink(&setup(strength, 0, bark));
            assert_eq!(
                next.combat.as_ref().unwrap().player.powers.strength,
                strength + if bark { 4 } else { 2 }
            );
        }
    }
}

#[test]
fn malformed_overflow_still_rejects_atomically_with_the_existing_error() {
    let initial = setup(i32::MAX, 0, false);
    let before = serde_json::to_value(&initial).unwrap();
    assert_eq!(
        apply_potion_action(
            &initial,
            RunAction::UsePotion {
                slot: 0,
                target: None
            }
        ),
        Err(SimError::InvalidState(
            "combat potion stat gain overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&initial).unwrap(), before);
}

#[test]
fn potion_bound_controls_following_strike_damage() {
    let next = drink(&setup(999, 0, false));
    let combat = next.combat.as_ref().unwrap();
    let attacked = apply_combat_action(
        combat,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(combat.monsters[0].id),
        },
    )
    .unwrap();
    assert_eq!(combat.monsters[0].hp - attacked.monsters[0].hp, 6 + 999);
}
