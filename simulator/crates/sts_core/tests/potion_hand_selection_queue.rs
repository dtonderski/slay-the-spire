//! Source-backed synthetic potion-selection barriers, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, Potion,
    Relic, RunAction, RunState,
};
use sts_core::content::cards::STRIKE_R_ID;
fn setup(relics: Vec<Relic>, potions: Vec<Potion>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    r.potions = potions;
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = (100..103)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.draw_pile = (200..220)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: Option<RunAction>) -> RunState {
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let apply = |r: &RunState| match a {
        Some(a) => apply_run_action(r, a),
        None => apply_combat_action_on_run(r, CombatAction::EndTurn),
    };
    let next = apply(r).unwrap();
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply(&restored).unwrap()).unwrap()
    );
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    next.validate().unwrap();
    println!(
        "potion_selection_transition={}",
        serde_json::json!({"initial":before,"run_action":a,"combat_action":a.is_none().then_some(CombatAction::EndTurn),"result":next})
    );
    next
}
fn drink(r: &RunState) -> RunState {
    let slot = (0..r.potion_capacity())
        .find(|s| r.potion_at_slot(*s).is_some())
        .unwrap();
    step(r, Some(RunAction::UsePotion { slot, target: None }))
}
fn confirm(r: &RunState, _p: Potion) -> RunState {
    // GamblingChip uses the legacy ExhaustSelectState with GamblingChip purpose.
    step(r, Some(RunAction::ConfirmExhaustSelect))
}
fn barrier_case(barrier: u8, p: Potion) {
    let opened = match barrier {
        0 => drink(&setup(vec![], vec![Potion::Attack, p])),
        1 => setup(vec![Relic::Toolbox], vec![p]),
        2 => step(&setup(vec![Relic::NilrysCodex], vec![p]), None),
        _ => unreachable!(),
    };
    assert!(opened
        .combat
        .as_ref()
        .unwrap()
        .combat_card_reward_choices()
        .is_some());
    let pending = drink(&opened);
    let a = opened.combat.as_ref().unwrap();
    let b = pending.combat.as_ref().unwrap();
    assert_eq!(
        b.combat_card_reward_choices(),
        a.combat_card_reward_choices(),
        "hand-selection potion must not replace the current reward"
    );
    assert_eq!(a.piles, b.piles);
    assert_eq!(a.rng, b.rng);
    let selected = step(
        &pending,
        Some(if barrier == 2 {
            RunAction::SkipCombatCardReward
        } else {
            RunAction::ChooseCombatCardReward { index: 0 }
        }),
    );
    let c = selected.combat.as_ref().unwrap();
    assert_eq!(c.monsters[0].moves_executed, 0);
    let select = c.exhaust_select().expect("queued potion selection");
    assert_eq!(
        select.purpose,
        if p == Potion::Elixir {
            sts_core::combat::ExhaustSelectPurpose::Exhaust
        } else {
            sts_core::combat::ExhaustSelectPurpose::GamblingChip
        }
    );
    let next = confirm(&selected, p);
    if barrier == 2 {
        let c = next.combat.as_ref().unwrap();
        assert_eq!(c.monsters[0].moves_executed, 1);
        assert!(!c.resume_end_turn_after_nilrys_codex);
    }
}
#[test]
fn elixir_waits_for_potion_reward() {
    barrier_case(0, Potion::Elixir);
}
#[test]
fn elixir_waits_for_toolbox_reward() {
    barrier_case(1, Potion::Elixir);
}
#[test]
fn elixir_waits_for_nilry_reward() {
    barrier_case(2, Potion::Elixir);
}
#[test]
fn gambler_waits_for_potion_reward() {
    barrier_case(0, Potion::GamblersBrew);
}
#[test]
fn gambler_waits_for_toolbox_reward() {
    barrier_case(1, Potion::GamblersBrew);
}
#[test]
fn gambler_waits_for_nilry_reward() {
    barrier_case(2, Potion::GamblersBrew);
}
#[test]
fn earlier_swift_draws_are_in_the_live_elixir_selection() {
    let opened = drink(&setup(
        vec![],
        vec![Potion::Attack, Potion::Swift, Potion::Elixir],
    ));
    let swift = drink(&opened);
    let pending = drink(&swift);
    let selected = step(
        &pending,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    assert_eq!(selected.combat.as_ref().unwrap().piles.hand.len(), 7);
    let toggled = step(&selected, Some(RunAction::ChooseExhaustSelect { index: 6 }));
    let next = confirm(&toggled, Potion::Elixir);
    assert_eq!(next.combat.as_ref().unwrap().piles.exhaust_pile.len(), 1);
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 6);
}
#[test]
fn selector_use_heal_waits_for_confirmation() {
    for p in [Potion::Elixir, Potion::GamblersBrew] {
        let mut r = setup(vec![Relic::ToyOrnithopter], vec![p]);
        r.combat.as_mut().unwrap().player.hp = 30;
        let opened = drink(&r);
        assert_eq!(opened.combat.as_ref().unwrap().player.hp, 30);
        let next = confirm(&opened, p);
        assert_eq!(next.combat.as_ref().unwrap().player.hp, 35);
    }
}
#[test]
fn empty_hand_at_gambler_use_does_not_open_a_later_selector() {
    let mut r = setup(vec![], vec![Potion::Attack, Potion::GamblersBrew]);
    r.combat.as_mut().unwrap().piles.hand.clear();
    let opened = drink(&r);
    let pending = drink(&opened);
    let next = step(
        &pending,
        Some(RunAction::ChooseCombatCardReward { index: 0 }),
    );
    assert!(next.combat.as_ref().unwrap().decision.is_none());
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 1);
}
#[test]
fn immediate_elixir_control_still_opens_a_selection() {
    let opened = drink(&setup(vec![], vec![Potion::Elixir]));
    assert!(opened.combat.as_ref().unwrap().exhaust_select().is_some());
    confirm(&opened, Potion::Elixir);
}
#[test]
fn immediate_gambler_control_still_opens_a_selection() {
    let opened = drink(&setup(vec![], vec![Potion::GamblersBrew]));
    assert!(opened.combat.as_ref().unwrap().exhaust_select().is_some());
    confirm(&opened, Potion::GamblersBrew);
}
