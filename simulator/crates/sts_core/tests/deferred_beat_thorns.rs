//! Initialized legacy Beat counter, not naturally-produced counter or dedicatedtrace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, legal_run_decision_actions, CardId, CardInstance,
    CombatAction, Relic, RunAction, RunDecisionAction, RunState,
};
use sts_core::content::{
    cards::{BURNING_PACT_ID, STRIKE_R_ID},
    monsters::CORRUPT_HEART_ID,
};
fn setup(relics: Vec<Relic>, triggers: u32) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    let c = r.combat.as_mut().unwrap();
    c.monsters[0].content_id = CORRUPT_HEART_ID;
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    c.monsters[0].powers.beat_of_death = 2;
    c.player.powers.plated_armor = 5;
    c.pending_beat_of_death_triggers = triggers;
    c.piles.hand = vec![
        CardInstance::new(CardId::new(100), BURNING_PACT_ID),
        CardInstance::new(CardId::new(101), STRIKE_R_ID),
        CardInstance::new(CardId::new(102), STRIKE_R_ID),
    ];
    c.piles.draw_pile = (200..210)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunDecisionAction) -> RunState {
    assert!(legal_run_decision_actions(r).unwrap().contains(&a));
    let before = serde_json::to_value(r).unwrap();
    let apply = |s: &RunState| match a {
        RunDecisionAction::Combat(a) => apply_combat_action_on_run(s, a),
        RunDecisionAction::Run(a) => apply_run_action(s, a),
        _ => unreachable!("fixture only uses combat/run actions"),
    };
    let n = apply(r).unwrap();
    n.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    let restored: RunState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        serde_json::to_value(apply(&restored).unwrap()).unwrap()
    );
    eprintln!("deferred_beat_transition={:?}", a);
    n
}
fn play_and_confirm(r: &RunState) -> RunState {
    let opened = step(
        r,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(100),
            target: None,
        }),
    );
    let selected = step(
        &opened,
        RunDecisionAction::Run(RunAction::ChooseExhaustSelect { index: 0 }),
    );
    step(
        &selected,
        RunDecisionAction::Run(RunAction::ConfirmExhaustSelect),
    )
}
#[test]
fn deferred_beat_does_not_reduce_plate() {
    let n = play_and_confirm(&setup(vec![], 1));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 5);
}
#[test]
fn deferred_beat_obeys_tungsten() {
    let n = play_and_confirm(&setup(vec![Relic::TungstenRod], 1));
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 78);
}
#[test]
fn multiple_old_triggers_settle_once() {
    let n = play_and_confirm(&setup(vec![], 2));
    let c = n.combat.as_ref().unwrap();
    assert_eq!(c.player.hp, 74);
    assert_eq!(c.player.powers.plated_armor, 5);
    assert_eq!(c.pending_beat_of_death_triggers, 0);
    let n = step(
        &n,
        RunDecisionAction::Combat(CombatAction::PlayCard {
            card_id: CardId::new(102),
            target: Some(sts_core::adapter_internals::MonsterId::new(1)),
        }),
    );
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 72);
}
#[test]
fn buffer_still_applies() {
    let mut r = setup(vec![], 1);
    r.combat.as_mut().unwrap().player.powers.buffer = 1;
    let n = play_and_confirm(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 78);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.buffer, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 5);
}
#[test]
fn intangible_still_caps() {
    let mut r = setup(vec![], 1);
    r.combat.as_mut().unwrap().player.powers.intangible = 1;
    let n = play_and_confirm(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 78);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 5);
}
#[test]
fn block_absorbs_both() {
    let mut r = setup(vec![], 1);
    r.combat.as_mut().unwrap().player.block = 4;
    let n = play_and_confirm(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 80);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 5);
}
#[test]
fn torii_not_applied_but_rod_is() {
    let n = play_and_confirm(&setup(vec![Relic::Torii, Relic::TungstenRod], 1));
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 78);
}
#[test]
fn old_beat_does_not_channel_static() {
    let mut r = setup(vec![], 1);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.static_discharge = 1;
    c.max_orbs = 3;
    let n = play_and_confirm(&r);
    assert!(n.combat.as_ref().unwrap().orbs.is_empty());
}
#[test]
fn ordinary_no_legacy_counter_control() {
    let n = play_and_confirm(&setup(vec![], 0));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 5);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 78);
}
