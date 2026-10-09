//! Source-backed synthetic tests, not dedicated real-game trace parity.
//! SteroidPotion.use applies bounded Strength, then nominal LoseStrengthPower;
//! Artifact blocks creation of the loss, not just its later expiry.
use sts_core::adapter_internals::{
    apply_combat_action, apply_potion_action, CombatAction, Potion, Relic, RunAction, RunState,
    SimError,
};

fn setup(base: i32, loss: i32, artifact: i32, bark: bool, potions: Vec<Potion>) -> RunState {
    let mut run = RunState::combat_fixture_with_relics(if bark {
        vec![Relic::SacredBark]
    } else {
        Vec::new()
    });
    run.empty_potion_slots = (potions.len()..3).collect();
    run.potions = potions;
    let c = run.combat.as_mut().unwrap();
    c.player.powers.strength = base;
    c.player.temp_strength = loss;
    c.player.powers.artifact = artifact;
    run.validate().unwrap();
    run
}

fn drink(run: &RunState, slot: usize) -> RunState {
    let action = RunAction::UsePotion { slot, target: None };
    let before = serde_json::to_value(run).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_potion_action(run, action).unwrap();
    let repeated = apply_potion_action(&restored, action).unwrap();
    assert_eq!(serde_json::to_value(run).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeated).unwrap()
    );
    assert_eq!(
        next.combat.as_ref().unwrap().rng,
        run.combat.as_ref().unwrap().rng
    );
    assert_eq!(next.potion_at_slot(slot), None);
    next.validate().unwrap();
    println!(
        "flex_potion_transition={}",
        serde_json::json!({
            "initial": before, "action": action, "result": next,
        })
    );
    next
}

fn expire(run: &RunState) -> (i32, i32, i32) {
    let c = apply_combat_action(run.combat.as_ref().unwrap(), CombatAction::EndTurn).unwrap();
    (
        c.player.powers.strength,
        c.player.temp_strength,
        c.player.powers.artifact,
    )
}

#[test]
fn flex_potion_caps_strength_but_expires_the_full_nominal_loss() {
    for bark in [false, true] {
        let amount = if bark { 10 } else { 5 };
        let next = drink(&setup(998, 0, 0, bark, vec![Potion::Flex]), 0);
        let c = next.combat.as_ref().unwrap();
        assert_eq!(c.player.powers.strength + c.player.temp_strength, 999);
        assert_eq!(c.player.temp_strength, amount);
        assert_eq!(expire(&next), (999 - amount, 0, 0));
    }
}

#[test]
fn artifact_blocks_loss_creation_immediately_and_gain_is_bounded() {
    for bark in [false, true] {
        let next = drink(&setup(998, 0, 1, bark, vec![Potion::Flex]), 0);
        let c = next.combat.as_ref().unwrap();
        assert_eq!(c.player.powers.artifact, 0);
        assert_eq!(c.player.temp_strength, 0);
        assert_eq!(c.player.powers.strength, 999);
        assert_eq!(expire(&next), (999, 0, 0));
    }
}

#[test]
fn blocking_new_loss_does_not_cancel_existing_loss() {
    let next = drink(&setup(996, 2, 1, false, vec![Potion::Flex]), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.powers.artifact, 0);
    assert_eq!(c.player.temp_strength, 2);
    assert_eq!(c.player.powers.strength + c.player.temp_strength, 999);
    assert_eq!(expire(&next), (997, 0, 0));
}

#[test]
fn one_artifact_protects_only_the_first_of_two_flex_potions() {
    let first = drink(&setup(0, 0, 1, false, vec![Potion::Flex, Potion::Flex]), 0);
    let second = drink(&first, 1);
    assert_eq!(expire(&second), (5, 0, 0));
}

#[test]
fn ancient_potion_after_flex_still_blocks_expiry() {
    let first = drink(
        &setup(0, 0, 0, false, vec![Potion::Flex, Potion::Ancient]),
        0,
    );
    let protected = drink(&first, 1);
    assert_eq!(expire(&protected), (5, 0, 0));
}

#[test]
fn ordinary_unprotected_gain_and_nominal_stacks_are_unchanged() {
    let next = drink(&setup(10, 2, 0, false, vec![Potion::Flex]), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.powers.strength + c.player.temp_strength, 17);
    assert_eq!(c.player.temp_strength, 7);
    assert_eq!(expire(&next), (10, 0, 0));
}

#[test]
fn malformed_loss_overflow_remains_atomic() {
    let initial = setup(0, i32::MAX, 0, false, vec![Potion::Flex]);
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
    assert_eq!(serde_json::to_value(initial).unwrap(), before);
}
