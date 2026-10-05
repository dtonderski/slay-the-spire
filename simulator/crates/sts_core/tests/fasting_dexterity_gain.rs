//! Source-backed positive gains; EnergyDown Artifact handling is a separate fix.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
use sts_core::content::cards::FASTING_ANY_COLOR_ID;
fn setup(dex: i32, debt: i32, upgrade: bool) -> RunState {
    let mut r = RunState::combat_fixture();
    let c = r.combat.as_mut().unwrap();
    c.player.energy = 5;
    c.player.powers.dexterity = dex;
    c.player.temp_dexterity = debt;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    c.piles.hand[0].content_id = FASTING_ANY_COLOR_ID;
    c.piles.hand[0].upgrades = u8::from(upgrade);
    c.piles.hand[1].content_id = FASTING_ANY_COLOR_ID;
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunDecisionAction) -> RunState {
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let initial = serde_json::to_value(r).unwrap();
    let restore: RunState = serde_json::from_value(initial.clone()).unwrap();
    let n = apply_run_decision_action(r, a).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), initial);
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply_run_decision_action(&restore, a).unwrap()).unwrap()
    );
    println!(
        "fasting_dex_transition={}",
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
fn second_fasting_gains_dexterity() {
    assert_eq!(
        play(&play(&setup(0, 0, false)))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        6
    );
}
#[test]
fn upgraded_then_ordinary_fasting_gains_both_amounts() {
    assert_eq!(
        play(&play(&setup(0, 0, true)))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        7
    );
}
#[test]
fn repeated_gains_retain_existing_nominal_loss() {
    let n = play(&play(&setup(10, 5, false)));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 16);
    assert_eq!(p.temp_dexterity, 5);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 11);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 1);
}
#[test]
fn repeated_gains_keep_actual_cap() {
    assert_eq!(
        play(&play(&setup(997, 0, false)))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        999
    );
}
#[test]
fn repeated_gains_stack_on_negative_power() {
    assert_eq!(
        play(&play(&setup(-9, 0, false)))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        -3
    );
}
#[test]
fn both_gains_affect_following_defend() {
    let mut r = setup(0, 0, false);
    r.combat.as_mut().unwrap().piles.hand[2].content_id = sts_core::content::cards::DEFEND_R_ID;
    let n = play(&play(&r));
    let id = n.combat.as_ref().unwrap().piles.hand[0].id;
    let n = step(
        &n,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: id,
            target: None,
        }),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.block, 11);
}
#[test]
fn repeated_casts_preserve_rng_and_declared_resources() {
    let r = setup(0, 0, false);
    let n = play(&play(&r));
    assert_eq!(
        serde_json::to_value(&n.combat.as_ref().unwrap().rng).unwrap(),
        serde_json::to_value(&r.combat.as_ref().unwrap().rng).unwrap()
    );
    assert_eq!(n.card_rng_counter, r.card_rng_counter);
    assert_eq!(n.card_random_rng_counter, r.card_random_rng_counter);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 6);
    assert_eq!(p.powers.dexterity, 6);
    assert_eq!(p.powers.fasting, 2);
    assert_eq!(p.energy, 1);
    assert!(!n
        .combat
        .as_ref()
        .unwrap()
        .piles
        .hand
        .iter()
        .any(|c| c.content_id == FASTING_ANY_COLOR_ID));
}
#[test]
fn first_fasting_control() {
    assert_eq!(
        play(&setup(0, 0, false))
            .combat
            .unwrap()
            .player
            .powers
            .dexterity,
        3
    );
}
