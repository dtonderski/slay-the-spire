//! Source-backed synthetic Lagavulin END prefixes, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::combat::CombatState;
fn setup(dex: i32, artifact: i32, ascension: u8) -> RunState {
    let mut r = RunState::combat_fixture_with_ascension(ascension);
    let c = r.combat.as_mut().unwrap();
    c.monsters = CombatState::lagavulin_fixture().monsters;
    c.player.powers.dexterity = dex;
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
        "siphon_order_transition={}",
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
fn one_artifact_blocks_dexterity_before_strength() {
    let n = first_siphon(setup(10, 1, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.powers.strength, -1);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn one_artifact_at_negative_cap_blocks_dexterity() {
    let n = first_siphon(setup(-999, 1, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, -999);
    assert_eq!(p.powers.strength, -1);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn no_artifact_control_applies_both() {
    let n = first_siphon(setup(10, 0, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 9);
    assert_eq!(p.powers.strength, -1);
}
#[test]
fn three_artifacts_retain_one_after_both_rejections() {
    let n = first_siphon(setup(10, 3, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.powers.strength, 0);
    assert_eq!(p.powers.artifact, 1);
}
#[test]
fn dexterity_rejection_preserves_existing_negative_strength_application() {
    let mut r = setup(10, 1, 0);
    r.combat.as_mut().unwrap().player.powers.strength = -2;
    let n = first_siphon(r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.powers.strength, -3);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn changed_debuff_recipient_preserves_ai_rng_draw_and_resources() {
    let n = first_siphon(setup(10, 1, 0));
    let m = first_siphon(setup(10, 2, 0));
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
fn two_artifacts_control_blocks_both() {
    let n = first_siphon(setup(10, 2, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.powers.strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
