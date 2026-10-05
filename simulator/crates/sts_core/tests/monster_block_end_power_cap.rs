//! Separate source-backed legal END prefixes for automatic monster block gains.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterIntent,
    RunDecisionAction, RunState,
};
use sts_core::content::monsters::SPHERIC_GUARDIAN_ID;
fn check(metallicize: i32, plated: i32) {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    let m = &mut c.monsters[0];
    m.content_id = SPHERIC_GUARDIAN_ID;
    m.block = 998;
    m.intent = MonsterIntent::AttackAndBlock {
        damage: 10,
        block: 15,
    };
    m.powers.metallicize = metallicize;
    m.powers.plated_armor = plated;
    r.validate().unwrap();
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert!(legal_run_decision_actions(&r).unwrap().contains(&a));
    let before = serde_json::to_value(&r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let n = apply_run_decision_action(&r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    println!(
        "monster_end_block_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    assert_eq!(n.combat.unwrap().monsters[0].block, 999);
}
#[test]
fn metallicize_caps_monster_block() {
    check(3, 0);
}
#[test]
fn plated_armor_caps_monster_block() {
    check(0, 3);
}
#[test]
fn sequential_end_powers_each_cap_monster_block() {
    check(3, 3);
}
