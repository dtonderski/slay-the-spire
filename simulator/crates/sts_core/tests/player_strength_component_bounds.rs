//! Source-backed initialized leftover-end diagnostics, not natural trace parity.
//! StrengthPower is the combined amount; nominal loss is separate bookkeeping.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterIntent,
    RunDecisionAction, RunState,
};
fn setup(permanent: i32, debt: i32, artifact: i32, shield: bool) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.strength = permanent;
    c.player.temp_strength = debt;
    c.player.powers.artifact = artifact;
    c.time_warp_end_turn_pre_discard_settled = true;
    c.preserve_temp_strength_on_next_start = true;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    c.monsters[0].content_id = if shield {
        sts_core::content::monsters::SPIRE_SHIELD_ID
    } else {
        sts_core::content::monsters::LAGAVULIN_ID
    };
    c.monsters[0].intent = if shield {
        MonsterIntent::AttackApplyPlayerWeak { damage: 0, weak: 0 }
    } else {
        MonsterIntent::SiphonPlayer {
            strength: 1,
            dexterity: 1,
        }
    };
    r.validate().unwrap();
    r
}
fn end(r: &RunState) -> RunState {
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restored, a).unwrap()).unwrap()
    );
    n
}
#[test]
fn siphon_reduces_actual_strength_not_the_permanent_component() {
    let n = end(&setup(-1005, 10, 0, false));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, -1006);
    assert_eq!(p.temp_strength, 10);
    assert_eq!(p.powers.strength + p.temp_strength, -996);
}
#[test]
fn shield_reduces_actual_strength_with_retained_loss() {
    let n = end(&setup(-1005, 10, 0, true));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, -1006);
    assert_eq!(p.temp_strength, 10);
}
#[test]
fn negative_cap_keeps_nominal_loss_component() {
    let n = end(&setup(-1009, 10, 0, false));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, -1009);
    assert_eq!(p.temp_strength, 10);
    assert_eq!(p.powers.strength + p.temp_strength, -999);
}
#[test]
fn combined_overflow_rejects_the_whole_transition() {
    let r = setup(i32::MAX, 1, 0, false);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::EndTurn);
    assert_eq!(
        apply_run_decision_action(&r, a).unwrap_err().to_string(),
        "invalid state: player combined Strength overflows i32"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn artifact_rejects_before_malformed_combined_arithmetic() {
    let n = end(&setup(i32::MAX, 1, 2, false));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, i32::MAX);
    assert_eq!(p.temp_strength, 1);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn artifact_rejects_without_repairing_the_component() {
    let n = end(&setup(-1005, 10, 2, false));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, -1005);
    assert_eq!(p.temp_strength, 10);
    assert_eq!(p.powers.artifact, 0);
}
