//! Source-backed Fasting -> Skill -> Attack Orange Pellets prefixes.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
use sts_core::content::cards::{BATTLE_TRANCE_ID, FASTING_ANY_COLOR_ID, STRIKE_R_ID};
use sts_core::relic::Relic;
fn setup(pellets: bool, focus: i32) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(if pellets {
        vec![Relic::OrangePellets]
    } else {
        vec![]
    });
    let c = r.combat.as_mut().unwrap();
    c.player.powers.focus = focus;
    c.monsters[0].hp = 4000;
    c.monsters[0].max_hp = 4000;
    c.piles.hand[0].content_id = FASTING_ANY_COLOR_ID;
    c.piles.hand[1].content_id = BATTLE_TRANCE_ID;
    c.piles.hand[2].content_id = STRIKE_R_ID;
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
        "focus_cleanse_transition={}",
        serde_json::json!({"initial":initial,"action":a,"result":n})
    );
    n
}
fn cast(r: &RunState, content: sts_core::adapter_internals::ContentId) -> RunState {
    let c = r.combat.as_ref().unwrap();
    let id = c
        .piles
        .hand
        .iter()
        .find(|c| c.content_id == content)
        .unwrap()
        .id;
    step(
        r,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: id,
            target: if content == STRIKE_R_ID {
                Some(c.monsters[0].id)
            } else {
                None
            },
        }),
    )
}
fn prefix(r: &RunState) -> RunState {
    cast(
        &cast(&cast(r, FASTING_ANY_COLOR_ID), BATTLE_TRANCE_ID),
        STRIKE_R_ID,
    )
}

#[test]
fn negative_focus_is_removed_after_full_pellets_set() {
    let n = prefix(&setup(true, -3));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, 0);
}
#[test]
fn negative_limit_focus_is_removed_without_affecting_positive_gains() {
    let n = prefix(&setup(true, -999));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.focus, 0);
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.powers.dexterity, 3);
}
#[test]
fn no_pellets_control_keeps_negative_focus() {
    let n = prefix(&setup(false, -3));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, -3);
}
#[test]
fn incomplete_card_type_set_preserves_focus_until_attack() {
    let n = cast(&setup(true, -3), FASTING_ANY_COLOR_ID);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, -3);
    let n = cast(&n, BATTLE_TRANCE_ID);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, -3);
    let n = cast(&n, STRIKE_R_ID);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, 0);
}
#[test]
fn raw_negative_constructor_amount_is_also_a_debuff() {
    let n = prefix(&setup(true, -1100));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, 0);
}
#[test]
fn zero_focus_control_remains_zero() {
    let n = prefix(&setup(true, 0));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, 0);
}
#[test]
fn removal_preserves_resources_draw_rng_and_positive_buff_gains() {
    let n = prefix(&setup(true, -3));
    let m = prefix(&setup(true, 3));
    let c = n.combat.as_ref().unwrap();
    let d = m.combat.as_ref().unwrap();
    assert_eq!(c.piles, d.piles);
    assert_eq!(c.monsters, d.monsters);
    assert_eq!(
        serde_json::to_value(&c.rng).unwrap(),
        serde_json::to_value(&d.rng).unwrap()
    );
    assert_eq!(c.player.hp, d.player.hp);
    assert_eq!(c.player.energy, 0);
    assert_eq!(c.player.energy, d.player.energy);
    assert_eq!(c.player.powers.strength, d.player.powers.strength);
    assert_eq!(c.player.powers.dexterity, d.player.powers.dexterity);
}
#[test]
fn positive_focus_control_is_not_a_debuff() {
    let n = prefix(&setup(true, 3));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.focus, 3);
}
