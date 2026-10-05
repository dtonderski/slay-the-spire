//! Explicit initialized Shield/Focus setups; legal END prefixes, not vanilla generation or trace parity.
use sts_core::adapter_internals::run::map::enter_synthetic_combat;
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, MonsterIntent, RoomKind,
    RunDecisionAction, RunPhase, RunState,
};
use sts_core::content::monsters::SPIRE_SHIELD_ID;
fn setup(focus: i32) -> RunState {
    let mut r = RunState::try_seeded_ironclad(0, 0).unwrap();
    r.phase = RunPhase::Idle;
    r.event = None;
    r.map = None;
    r.emerald_key_node = None;
    r.current_act = 4;
    r.run_player.as_mut().unwrap().hp = 400;
    r.run_player.as_mut().unwrap().max_hp = 400;
    enter_synthetic_combat(&mut r, RoomKind::Elite, "Shield and Spear").unwrap();
    let c = r.combat.as_mut().unwrap();
    c.player.powers.focus = focus;
    c.max_orbs = 1;
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
        "focus_reduction_transition={}",
        serde_json::json!({"initial":initial,"action":a,"result":n})
    );
    n
}
fn first_focus_branch(mut r: RunState) -> RunState {
    for _ in 0..12 {
        let c = r.combat.as_ref().unwrap();
        let smash = matches!(
            c.monsters
                .iter()
                .find(|m| m.content_id == SPIRE_SHIELD_ID)
                .unwrap()
                .intent,
            MonsterIntent::AttackApplyPlayerWeak { weak: 0, .. }
        );
        let strength = c.player.powers.strength;
        r = end(&r);
        // With no Artifact and ordinary Strength, unchanged Strength identifies
        // the naturally selected Focus branch. Do not force/replace RNG or intent.
        if smash && r.combat.as_ref().unwrap().player.powers.strength == strength {
            return r;
        }
    }
    panic!("default initialized Shield stream did not take Focus branch within12 ENDs");
}
#[test]
fn negative_limit_caps_existing_focus_stack() {
    assert_eq!(
        first_focus_branch(setup(-999))
            .combat
            .unwrap()
            .player
            .powers
            .focus,
        -999
    );
}
#[test]
fn near_limit_repeated_natural_focus_applications_cap() {
    let mut r = setup(-998);
    // Declared initial endurance HP for two naturally chosen Focus branches;
    // never replenish HP after an accepted action or from an observation.
    r.combat.as_mut().unwrap().player.hp = 800;
    r.combat.as_mut().unwrap().player.max_hp = 800;
    r.validate().unwrap();
    let n = first_focus_branch(first_focus_branch(r));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, -999);
}
#[test]
fn cap_preserves_ai_stream_draw_and_other_resources() {
    let n = first_focus_branch(setup(-999));
    let m = first_focus_branch(setup(-996));
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
fn raw_constructor_then_existing_stack_is_not_global_repair() {
    let r = setup(0);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    assert!(sts_core::power::reduce_player_focus(&mut p, 1100).unwrap());
    assert_eq!(p.focus, -1100);
    assert!(sts_core::power::reduce_player_focus(&mut p, 1).unwrap());
    assert_eq!(p.focus, -999);
}
#[test]
fn initialized_positive_constructor_amount_caps_on_later_stack() {
    let r = setup(1100);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    assert!(sts_core::power::reduce_player_focus(&mut p, 1).unwrap());
    assert_eq!(p.focus, 999);
}
#[test]
fn artifact_blocks_before_malformed_arithmetic_without_repair() {
    let r = setup(i32::MIN);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    p.artifact = 1;
    assert!(!sts_core::power::reduce_player_focus(&mut p, 1).unwrap());
    assert_eq!(p.focus, i32::MIN);
    assert_eq!(p.artifact, 0);
}
#[test]
fn exact_underflow_keeps_entire_power_input() {
    let r = setup(i32::MIN);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    let before = p;
    assert_eq!(
        sts_core::power::reduce_player_focus(&mut p, 1),
        Err(sts_core::adapter_internals::SimError::InvalidState(
            "player Focus reduction underflows i32"
        ))
    );
    assert_eq!(p, before);
}
#[test]
fn zero_and_negative_reduction_do_not_repair_initialized_amount() {
    let r = setup(-1100);
    let mut p = r.combat.as_ref().unwrap().player.powers;
    let before = p;
    assert!(!sts_core::power::reduce_player_focus(&mut p, 0).unwrap());
    assert!(!sts_core::power::reduce_player_focus(&mut p, -1).unwrap());
    assert_eq!(p, before);
}
#[test]
fn positive_control() {
    assert_eq!(
        first_focus_branch(setup(10))
            .combat
            .unwrap()
            .player
            .powers
            .focus,
        9
    );
}
#[test]
fn ordinary_negative_control() {
    assert_eq!(
        first_focus_branch(setup(-996))
            .combat
            .unwrap()
            .player
            .powers
            .focus,
        -997
    );
}
