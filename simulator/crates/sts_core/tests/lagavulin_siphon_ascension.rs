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
        "laga_potency_transition={}",
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
fn a18_siphon_loses_two_of_each_stat() {
    let n = first_siphon(setup(10, 0, 18));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 8);
    assert_eq!(p.powers.strength, -2);
}
#[test]
fn a20_siphon_loses_two_of_each_stat() {
    let n = first_siphon(setup(10, 0, 20));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 8);
    assert_eq!(p.powers.strength, -2);
}
#[test]
fn a18_one_artifact_blocks_full_dexterity_loss_then_strength_applies() {
    let n = first_siphon(setup(10, 1, 18));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.powers.strength, -2);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn a0_control_loses_one_of_each_stat() {
    let n = first_siphon(setup(10, 0, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 9);
    assert_eq!(p.powers.strength, -1);
}
#[test]
fn published_siphon_intent_matches_ascension_potency() {
    for (asc, amount) in [(0, 1), (17, 1), (18, 2), (20, 2)] {
        let mut r = setup(10, 0, asc);
        for i in 0..12 {
            if let MonsterIntent::SiphonPlayer {
                strength,
                dexterity,
            } = r.combat.as_ref().unwrap().monsters[0].intent
            {
                assert_eq!((strength, dexterity), (amount, amount));
                break;
            }
            assert!(i < 11, "fixture did not publish Siphon within bound");
            r = end(&r);
        }
    }
}
#[test]
fn two_artifacts_reject_two_whole_ascended_applications() {
    let n = first_siphon(setup(10, 2, 18));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.powers.strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn potency_preserves_other_cycle_resources_rng_and_draw() {
    let n = first_siphon(setup(10, 0, 18));
    let m = first_siphon(setup(10, 0, 17));
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
fn a17_control_loses_one_of_each_stat() {
    let n = first_siphon(setup(10, 0, 17));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 9);
    assert_eq!(p.powers.strength, -1);
}
