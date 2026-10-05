//! Source-backed Fasting -> Skill -> Attack Orange Pellets prefixes.
use sts_core::adapter_internals::{
    apply_run_decision_action, legal_run_decision_actions, CombatAction, RunDecisionAction,
    RunState,
};
use sts_core::content::cards::{BATTLE_TRANCE_ID, FASTING_ANY_COLOR_ID, STRIKE_R_ID};
use sts_core::relic::Relic;
fn setup(pellets: bool, dex: i32) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(if pellets {
        vec![Relic::OrangePellets]
    } else {
        vec![]
    });
    let c = r.combat.as_mut().unwrap();
    c.player.powers.dexterity = dex;
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
        "energy_cleanse_transition={}",
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
fn pellets_removes_energy_down_preserving_positive_gains() {
    let n = prefix(&setup(true, 0));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.fasting, 0);
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.powers.dexterity, 3);
    assert!(!p.cannot_draw);
}
#[test]
fn pellets_removes_energy_down_when_dexterity_stays_negative() {
    let n = prefix(&setup(true, -10));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.fasting, 0);
    assert_eq!(p.powers.dexterity, 0);
    assert_eq!(p.powers.strength, 3);
}
#[test]
fn power_and_skill_do_not_cleanse_until_attack_completes_set() {
    let n = cast(&setup(true, 0), FASTING_ANY_COLOR_ID);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.fasting, 1);
    let n = cast(&n, BATTLE_TRANCE_ID);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.fasting, 1);
    let n = cast(&n, STRIKE_R_ID);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.fasting, 0);
}
#[test]
fn whole_existing_energy_down_stack_is_removed_without_refund() {
    let mut r = setup(true, 0);
    r.combat.as_mut().unwrap().player.powers.fasting = 2;
    let n = prefix(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.fasting, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 3);
}
#[test]
fn cleanse_changes_only_future_turn_energy_penalty() {
    let n = prefix(&setup(true, 0));
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 0);
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 3);
    let n = prefix(&setup(false, 0));
    let n = step(&n, RunDecisionAction::Combat(CombatAction::EndTurn));
    assert_eq!(n.combat.as_ref().unwrap().player.energy, 2);
}
#[test]
fn without_pellets_control_keeps_energy_down() {
    assert_eq!(
        prefix(&setup(false, 0))
            .combat
            .unwrap()
            .player
            .powers
            .fasting,
        1
    );
}
