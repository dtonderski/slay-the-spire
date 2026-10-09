//! Synthetic enumerated legal card prefixes; not dedicated real-game parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    ContentId, MonsterId, MonsterIntent, RunDecisionAction, RunState,
};
use sts_core::content::cards::{DARK_SHACKLES_ID, DISARM_ID, DISARM_PLUS_ID};
fn setup(card: ContentId, strength: i32, debt: i32, artifact: i32) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![CardInstance::new(CardId::new(100), card)];
    let m = &mut c.monsters[0];
    m.hp = 100;
    m.max_hp = 100;
    m.intent = MonsterIntent::Block { block: 0 };
    m.powers.strength = strength;
    m.temp_strength_down = debt;
    m.powers.artifact = artifact;
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunDecisionAction) -> RunState {
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
    println!(
        "monster_debuff_transition={}",
        serde_json::json!({"initial":before,"action":a,"result":n})
    );
    n
}
fn play(r: &RunState) -> RunState {
    step(
        r,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: Some(MonsterId::new(1)),
        }),
    )
}
fn strength(r: &RunState) -> i32 {
    r.combat.as_ref().unwrap().monsters[0].powers.strength
}
#[test]
fn disarm_caps_negative_strength() {
    let r = setup(DISARM_ID, -998, 0, 0);
    assert_eq!(strength(&play(&r)), -999);
}
#[test]
fn upgraded_disarm_caps_negative_strength() {
    let r = setup(DISARM_PLUS_ID, -998, 0, 0);
    assert_eq!(strength(&play(&r)), -999);
}
#[test]
fn shackles_caps_negative_strength_then_restores_nominal_amount() {
    let r = setup(DARK_SHACKLES_ID, -995, 0, 0);
    let n = play(&r);
    assert_eq!(strength(&n), -999);
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].temp_strength_down, 9);
    let after = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(strength(&after), -990);
    assert_eq!(after.combat.unwrap().monsters[0].temp_strength_down, 0);
}
#[test]
fn shackled_stacking_caps_the_separate_power() {
    let r = setup(DARK_SHACKLES_ID, 10, 995, 0);
    let n = play(&r);
    assert_eq!(strength(&n), 1);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 999);
}
#[test]
fn already_capped_shackled_remains_capped() {
    let r = setup(DARK_SHACKLES_ID, 10, 999, 0);
    let n = play(&r);
    assert_eq!(strength(&n), 1);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 999);
}
#[test]
fn artifact_blocks_disarm_without_clamping_existing_strength_control() {
    let r = setup(DISARM_ID, -998, 0, 1);
    let n = play(&r);
    assert_eq!(strength(&n), -998);
    assert_eq!(n.combat.unwrap().monsters[0].powers.artifact, 0);
}
#[test]
fn artifact_blocks_shackles_without_adding_debt_control() {
    let r = setup(DARK_SHACKLES_ID, -998, 990, 1);
    let n = play(&r);
    assert_eq!(strength(&n), -998);
    let m = &n.combat.as_ref().unwrap().monsters[0];
    assert_eq!(m.temp_strength_down, 990);
    assert_eq!(m.powers.artifact, 0);
}
#[test]
fn ordinary_shackles_control() {
    let r = setup(DARK_SHACKLES_ID, 3, 0, 0);
    let n = play(&r);
    assert_eq!(strength(&n), -6);
    assert_eq!(n.combat.unwrap().monsters[0].temp_strength_down, 9);
}
#[test]
fn capped_shackled_expiry_uses_its_capped_stack() {
    let r = setup(DARK_SHACKLES_ID, 10, 995, 0);
    let n = play(&r);
    let after = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(strength(&after), 999);
    assert_eq!(after.combat.unwrap().monsters[0].temp_strength_down, 0);
}
#[test]
fn disarm_at_negative_cap_still_applies_debuff_callback() {
    let mut r = setup(DISARM_ID, -999, 0, 0);
    r.combat.as_mut().unwrap().player.powers.sadistic_nature = 5;
    let n = play(&r);
    assert_eq!(strength(&n), -999);
    assert_eq!(n.combat.unwrap().monsters[0].hp, 95);
}
#[test]
fn malformed_reduction_overflow_keeps_error_and_input() {
    let r = setup(DISARM_ID, i32::MIN, 0, 0);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: Some(MonsterId::new(1)),
    });
    assert_eq!(
        apply_run_decision_action(&r, a).unwrap_err().to_string(),
        "invalid state: monster Strength reduction underflows i32"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn malformed_debt_overflow_keeps_error_and_input() {
    let r = setup(DARK_SHACKLES_ID, 10, i32::MAX, 0);
    let before = serde_json::to_value(&r).unwrap();
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: Some(MonsterId::new(1)),
    });
    assert_eq!(
        apply_run_decision_action(&r, a).unwrap_err().to_string(),
        "invalid state: combat integer addition overflows i32"
    );
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
