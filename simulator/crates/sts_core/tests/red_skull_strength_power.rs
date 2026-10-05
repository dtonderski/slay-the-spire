//! Source-backed synthetic tests, not dedicated real-game trace parity.
//! RedSkull.onNotBloodied applies negative StrengthPower; Artifact can block it.
use sts_core::adapter_internals::{
    apply_combat_action, apply_potion_action, CardId, CombatAction, Potion, Relic, RunAction,
    RunState,
};

fn setup(hp: i32, strength: i32, artifact: i32, skull: bool, bottles: usize) -> RunState {
    let mut run = RunState::combat_fixture_with_relics(if skull {
        vec![Relic::RedSkull]
    } else {
        Vec::new()
    });
    run.potions = vec![Potion::Blood; bottles];
    run.empty_potion_slots = (bottles..3).collect();
    let combat = run.combat.as_mut().unwrap();
    combat.player.hp = hp;
    combat.player.powers.strength = strength;
    combat.player.powers.artifact = artifact;
    combat.relic_counters.red_skull_active = skull;
    run.validate().unwrap();
    run
}

fn drink(initial: &RunState, slot: usize) -> RunState {
    let action = RunAction::UsePotion { slot, target: None };
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
        initial.combat.as_ref().unwrap().rng,
        next.combat.as_ref().unwrap().rng
    );
    assert_eq!(next.potion_at_slot(slot), None);
    next.validate().unwrap();
    println!(
        "red_skull_transition={}",
        serde_json::json!({"initial": before, "action": action, "result": next})
    );
    next
}

#[test]
fn blood_potion_red_skull_removal_is_blocked_by_artifact() {
    let next = drink(&setup(40, 3, 1, true, 1), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 56);
    assert!(!c.relic_counters.red_skull_active);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 0);
}

#[test]
fn blocked_removal_consumes_only_one_artifact() {
    let next = drink(&setup(40, 3, 2, true, 1), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 1);
    assert!(!c.relic_counters.red_skull_active);
}

#[test]
fn another_heal_does_not_retry_the_blocked_removal() {
    let first = drink(&setup(40, 3, 2, true, 2), 0);
    let next = drink(&first, 1);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 72);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 1);
    assert!(!c.relic_counters.red_skull_active);
}

#[test]
fn removal_without_artifact_is_unchanged() {
    let next = drink(&setup(40, 3, 0, true, 1), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.powers.strength, 0);
    assert_eq!(c.player.powers.artifact, 0);
    assert!(!c.relic_counters.red_skull_active);
}

#[test]
fn healing_to_exactly_half_hp_keeps_skull_and_artifact() {
    let next = drink(&setup(24, 3, 1, true, 1), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 40);
    assert!(c.relic_counters.red_skull_active);
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 1);
}

#[test]
fn healing_without_skull_does_not_consume_artifact() {
    let next = drink(&setup(40, 3, 1, false, 1), 0);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.player.powers.strength, 3);
    assert_eq!(c.player.powers.artifact, 1);
}

#[test]
fn retained_strength_affects_following_attack_damage() {
    let next = drink(&setup(40, 3, 1, true, 1), 0);
    let c = next.combat.as_ref().unwrap();
    let attacked = apply_combat_action(
        c,
        CombatAction::PlayCard {
            card_id: CardId::new(1),
            target: Some(c.monsters[0].id),
        },
    )
    .unwrap();
    assert_eq!(c.monsters[0].hp - attacked.monsters[0].hp, 9);
}
