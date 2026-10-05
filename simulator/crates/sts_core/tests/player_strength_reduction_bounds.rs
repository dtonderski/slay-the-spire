//! Source-backed synthetic Lagavulin END prefixes, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::combat::CombatState;
fn setup(strength: i32, artifact: i32, ascension: u8) -> RunState {
    let mut r = RunState::combat_fixture_with_ascension(ascension);
    let c = r.combat.as_mut().unwrap();
    c.monsters = CombatState::lagavulin_fixture().monsters;
    c.player.powers.strength = strength;
    c.player.powers.artifact = artifact;
    r.validate().unwrap();
    r
}
fn end(r: &RunState) -> RunState {
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
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
        "strength_reduction_transition={}",
        serde_json::json!({"initial":initial,"action":a,"result":n})
    );
    n
}
fn first_siphon(mut r: RunState) -> RunState {
    for _ in 0..12 {
        let siphon = matches!(
            r.combat.as_ref().unwrap().monsters[0].intent,
            MonsterIntent::SiphonPlayer { .. }
        );
        r = end(&r);
        if siphon {
            return r;
        }
    }
    panic!("fixture did not reach Siphon within12 ENDs");
}
#[test]
fn negative_limit_caps_a0_incoming_strength_loss() {
    assert_eq!(
        first_siphon(setup(-999, 0, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        -999
    );
}
#[test]
fn negative_limit_caps_a18_incoming_strength_loss() {
    assert_eq!(
        first_siphon(setup(-999, 0, 18))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        -999
    );
}
#[test]
fn near_limit_caps_ascended_strength_loss() {
    assert_eq!(
        first_siphon(setup(-998, 0, 18))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        -999
    );
}
#[test]
fn one_artifact_rejects_dexterity_then_strength_loss_caps() {
    let n = first_siphon(setup(-999, 1, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 0);
    assert_eq!(p.powers.strength, -999);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn positive_control_loses_full_amount() {
    assert_eq!(
        first_siphon(setup(10, 0, 18))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        8
    );
}
#[test]
fn temporary_component_expires_before_monster_strength_application() {
    let mut r = setup(-999, 0, 0);
    r.combat.as_mut().unwrap().player.temp_strength = 5;
    let n = first_siphon(r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, -999);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn cap_preserves_cycle_resources_draw_and_rng() {
    let n = first_siphon(setup(-999, 0, 0));
    let m = first_siphon(setup(-996, 0, 0));
    let c = n.combat.as_ref().unwrap();
    let d = m.combat.as_ref().unwrap();
    assert_eq!(c.monsters, d.monsters);
    assert_eq!(c.piles, d.piles);
    assert_eq!(
        serde_json::to_value(&c.rng).unwrap(),
        serde_json::to_value(&d.rng).unwrap()
    );
    assert_eq!(c.player.hp, d.player.hp);
    assert_eq!(c.player.energy, d.player.energy);
}
#[test]
fn malformed_subtraction_preserves_error_and_whole_input() {
    let mut r = setup(i32::MIN, 0, 0);
    for _ in 0..12 {
        if matches!(
            r.combat.as_ref().unwrap().monsters[0].intent,
            MonsterIntent::SiphonPlayer { .. }
        ) {
            let before = serde_json::to_value(&r).unwrap();
            assert_eq!(
                apply_run_decision_action(&r, RunDecisionAction::Combat(CombatAction::EndTurn)),
                Err(sts_core::adapter_internals::SimError::InvalidState(
                    "player Strength reduction underflows i32"
                ))
            );
            assert_eq!(serde_json::to_value(&r).unwrap(), before);
            return;
        }
        r = end(&r);
    }
    panic!("fixture did not reach Siphon within12 ENDs");
}
#[test]
fn two_artifacts_reject_strength_without_repairing_old_amount() {
    let n = first_siphon(setup(-1005, 2, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, -1005);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn zero_or_negative_reduction_does_not_repair_initialized_amount() {
    let r = setup(-1005, 1, 0);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    let before = p;
    assert!(!sts_core::power::reduce_player_strength(&mut p, 0).unwrap());
    assert!(!sts_core::power::reduce_player_strength(&mut p, -2).unwrap());
    assert_eq!(p, before);
}
#[test]
fn ordinary_negative_control() {
    assert_eq!(
        first_siphon(setup(-996, 0, 0))
            .combat
            .unwrap()
            .player
            .powers
            .strength,
        -997
    );
}
