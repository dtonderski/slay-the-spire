//! Synthetic legal card prefixes; source-backed, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CardId, CardInstance, CombatAction,
    MonsterId, RunDecisionAction, RunState,
};
use sts_core::content::cards::DARK_SHACKLES_ID;
fn setup(strength: i32, debt: i32, sadistic: i32, artifact: i32) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![CardInstance::new(CardId::new(100), DARK_SHACKLES_ID)];
    c.piles.draw_pile.clear();
    c.piles.discard_pile.clear();
    c.player.powers.sadistic_nature = sadistic;
    c.monsters.truncate(1);
    let m = &mut c.monsters[0];
    m.hp = 100;
    m.max_hp = 100;
    m.powers.strength = strength;
    m.powers.artifact = artifact;
    m.temp_strength_down = debt;
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
        "shackled_callback_transition={}",
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
#[test]
fn shackled_new_power_is_explicitly_excluded_from_debuff_damage() {
    let n = play(&setup(3, 0, 5, 0));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 95);
    assert_eq!(m.powers.strength, -6);
    assert_eq!(m.temp_strength_down, 9);
}
#[test]
fn shackled_existing_power_is_explicitly_excluded_from_debuff_damage() {
    let n = play(&setup(3, 9, 5, 0));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 95);
    assert_eq!(m.temp_strength_down, 18);
}
#[test]
fn shackled_capped_stack_does_not_invent_second_damage() {
    let n = play(&setup(10, 999, 5, 0));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 95);
    assert_eq!(m.temp_strength_down, 999);
}
#[test]
fn capped_negative_strength_triggers_damage_but_shackled_does_not() {
    let n = play(&setup(-999, 0, 5, 0));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 95);
    assert_eq!(m.powers.strength, -999);
    assert_eq!(m.temp_strength_down, 9);
}
#[test]
fn no_sadistic_nature_control() {
    let n = play(&setup(3, 0, 0, 0));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 100);
    assert_eq!(m.temp_strength_down, 9);
}
#[test]
fn artifact_control_does_not_invent_second_application() {
    let n = play(&setup(3, 0, 5, 2));
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 100);
    assert_eq!(m.powers.strength, 3);
    assert_eq!(m.temp_strength_down, 0);
    assert_eq!(m.powers.artifact, 1);
}
#[test]
fn excluded_second_callback_does_not_finish_combat() {
    let mut r = setup(3, 0, 5, 0);
    r.combat.as_mut().unwrap().monsters[0].hp = 7;
    let n = play(&r);
    assert_eq!(n.phase, sts_core::adapter_internals::RunPhase::Combat);
    assert!(n.reward.is_none());
    let m = &n.combat.unwrap().monsters[0];
    assert!(m.alive);
    assert_eq!(m.hp, 2);
}
#[test]
fn callbacks_consume_block_separately_and_ignore_player_weak() {
    let mut r = setup(3, 0, 5, 0);
    let c = r.combat.as_mut().unwrap();
    c.monsters[0].block = 5;
    c.player.powers.weak = 2;
    c.player.powers.strength = 100;
    let n = play(&r);
    let m = &n.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 100);
    assert_eq!(m.block, 0);
}
#[test]
fn positive_expiry_does_not_invent_another_debuff_callback() {
    let n = play(&setup(-995, 0, 5, 0));
    let after = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    let m = &after.combat.unwrap().monsters[0];
    assert_eq!(m.hp, 95);
    assert_eq!(m.powers.strength, -990);
    assert_eq!(m.temp_strength_down, 0);
}
