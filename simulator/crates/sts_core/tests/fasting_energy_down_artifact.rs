//! Source-backed legal prefixes; not dedicated cross-color trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
use sts_core::content::cards::FASTING_ANY_COLOR_ID;
fn setup(artifact: i32, existing: i32) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.energy = 5;
    c.player.powers.artifact = artifact;
    c.player.powers.fasting = existing;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    c.piles.hand[0].content_id = FASTING_ANY_COLOR_ID;
    c.piles.hand[1].content_id = FASTING_ANY_COLOR_ID;
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunDecisionAction) -> RunState {
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let initial = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(initial.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), initial);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    println!(
        "energy_down_transition={}",
        serde_json::json!({"initial":initial,"action":a,"result":n})
    );
    n
}
fn play(r: &RunState) -> RunState {
    let id = r
        .combat
        .as_ref()
        .unwrap()
        .piles
        .hand
        .iter()
        .find(|c| c.content_id == FASTING_ANY_COLOR_ID)
        .unwrap()
        .id;
    step(
        r,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: id,
            target: None,
        }),
    )
}
#[test]
fn artifact_blocks_new_loss_not_positive_gains() {
    let n = play(&setup(1, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.fasting, 0);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.powers.dexterity, 3);
}
#[test]
fn each_incoming_loss_consumes_one_artifact() {
    let n = play(&setup(2, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.artifact, 1);
    assert_eq!(p.powers.fasting, 0);
}
#[test]
fn rejecting_new_loss_retains_existing_loss() {
    let n = play(&setup(1, 1));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.fasting, 1);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.powers.dexterity, 3);
}
#[test]
fn second_fasting_after_rejection_creates_only_second_loss() {
    let n = play(&play(&setup(1, 0)));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.fasting, 1);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.powers.strength, 6);
    assert_eq!(p.powers.dexterity, 6);
}
#[test]
fn rejected_loss_preserves_next_turn_energy() {
    let n = play(&setup(1, 0));
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 3);
    let n = play(&setup(0, 0));
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 2);
}
#[test]
fn existing_loss_still_lowers_next_turn_energy_after_rejection() {
    let n = play(&setup(1, 1));
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 2);
}
#[test]
fn upgraded_gain_caps_before_energy_down_then_dex_loss_expires() {
    let mut r = setup(1, 0);
    let c = r.combat.as_mut().unwrap();
    c.piles.hand[0].upgrades = 1;
    c.player.powers.strength = 997;
    c.player.powers.dexterity = 998;
    c.player.temp_dexterity = 5;
    let n = play(&r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 999);
    assert_eq!(p.powers.dexterity, 999);
    assert_eq!(p.temp_dexterity, 5);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.powers.fasting, 0);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 994);
}
#[test]
fn blocked_debuff_preserves_rng_and_resources() {
    let r = setup(1, 1);
    let n = play(&r);
    assert_eq!(
        serde_json::to_value(&n.combat.as_ref().unwrap().rng).unwrap(),
        serde_json::to_value(&r.combat.as_ref().unwrap().rng).unwrap()
    );
    assert_eq!(n.card_rng_counter, r.card_rng_counter);
    assert_eq!(n.card_random_rng_counter, r.card_random_rng_counter);
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 3);
    assert_eq!(
        n.combat
            .as_ref()
            .unwrap()
            .piles
            .hand
            .iter()
            .filter(|c| c.content_id == FASTING_ANY_COLOR_ID)
            .count(),
        1
    );
}
#[test]
fn malformed_old_stack_overflow_is_atomic_without_artifact() {
    let r = setup(0, i32::MAX);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: r.combat.as_ref().unwrap().piles.hand[0].id,
        target: None,
    });
    assert!(legal_run_decision_actions(&r).unwrap().contains(&a));
    assert_eq!(
        apply_run_decision_action(&r, a),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn artifact_rejection_precedes_rejected_stack_arithmetic_without_repair() {
    let n = play(&setup(1, i32::MAX));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.fasting, i32::MAX);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.powers.dexterity, 3);
}
#[test]
fn unblocked_first_loss_control() {
    assert_eq!(play(&setup(0, 0)).combat.unwrap().player.powers.fasting, 1);
}
#[test]
fn unblocked_existing_loss_stacks_control() {
    assert_eq!(play(&setup(0, 2)).combat.unwrap().player.powers.fasting, 3);
}
