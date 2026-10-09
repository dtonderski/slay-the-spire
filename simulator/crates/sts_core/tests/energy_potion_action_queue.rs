//! Source-backed synthetic legal gameplay prefixes, not interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, Potion,
    Relic, RunAction, RunState,
};
use sts_core::content::cards::{SECRET_WEAPON_ID, STRIKE_R_ID};
fn setup(relics: Vec<Relic>, potions: Vec<Potion>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(relics);
    r.potions = potions;
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![CardInstance::new(CardId::new(100), STRIKE_R_ID)];
    c.piles.draw_pile = (200..220)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    c.monsters[0].hp = 1000;
    c.monsters[0].max_hp = 1000;
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: Option<RunAction>) -> RunState {
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let apply = |s: &RunState| match a {
        Some(a) => apply_run_action(s, a),
        None => apply_combat_action_on_run(s, CombatAction::EndTurn),
    };
    let next = apply(r).unwrap();
    next.validate().unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply(&restored).unwrap()).unwrap()
    );
    println!(
        "energy_potion_transition={}",
        serde_json::json!({"initial":before,"run_action":a,"end_turn":a.is_none(),"result":next})
    );
    next
}
fn drink(r: &RunState) -> RunState {
    let slot = (0..r.potion_capacity())
        .find(|s| r.potion_at_slot(*s).is_some())
        .unwrap();
    step(r, Some(RunAction::UsePotion { slot, target: None }))
}
fn choose(r: &RunState) -> RunState {
    step(r, Some(RunAction::ChooseCombatCardReward { index: 0 }))
}
#[test]
fn energy_waits_behind_potion_reward() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Energy]));
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.energy, 3);
    let next = choose(&pending);
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn energy_waits_behind_toolbox_reward() {
    let r = setup(vec![Relic::Toolbox], vec![Potion::Energy]);
    let pending = drink(&r);
    assert_eq!(pending.combat.as_ref().unwrap().player.energy, 3);
    let next = choose(&pending);
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn energy_waits_behind_nilry_and_is_reset_for_next_turn() {
    let r = setup(vec![Relic::NilrysCodex], vec![Potion::Energy]);
    let opened = step(&r, None);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.energy, 3);
    let next = step(&pending, Some(RunAction::SkipCombatCardReward));
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 3);
    assert_eq!(next.combat.as_ref().unwrap().monsters[0].moves_executed, 1);
}
#[test]
fn energy_waits_behind_elixir_selector() {
    let r = setup(vec![], vec![Potion::Elixir, Potion::Energy]);
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.energy, 3);
    let selected = step(&pending, Some(RunAction::ChooseExhaustSelect { index: 0 }));
    let next = step(&selected, Some(RunAction::ConfirmExhaustSelect));
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn energy_use_heal_waits_for_current_reward() {
    let mut r = setup(
        vec![Relic::ToyOrnithopter],
        vec![Potion::Attack, Potion::Energy],
    );
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
    let next = choose(&pending);
    assert_eq!(next.combat.as_ref().unwrap().player.hp, 40);
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn multiple_energy_batches_resume_once_in_fifo_order() {
    let mut r = setup(
        vec![Relic::ToyOrnithopter],
        vec![Potion::Attack, Potion::Energy, Potion::Energy],
    );
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.energy, 3);
    let pending = drink(&pending);
    let next = choose(&pending);
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 7);
    assert_eq!(next.combat.as_ref().unwrap().player.hp, 45);
    assert_eq!(
        next.card_random_rng_counter,
        next.combat.as_ref().unwrap().rng.card_random_rng.counter()
    );
}
#[test]
fn queued_energy_does_not_publish_unrelated_offer_rng() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Energy]));
    let counter = opened.card_random_rng_counter;
    let combat_counter = opened
        .combat
        .as_ref()
        .unwrap()
        .rng
        .card_random_rng
        .counter();
    let pending = drink(&opened);
    assert_eq!(pending.card_random_rng_counter, counter);
    assert_eq!(
        pending
            .combat
            .as_ref()
            .unwrap()
            .rng
            .card_random_rng
            .counter(),
        combat_counter
    );
}
#[test]
fn energy_waits_behind_secret_weapon_grid() {
    let mut r = setup(vec![], vec![Potion::Energy]);
    r.combat.as_mut().unwrap().piles.hand =
        vec![CardInstance::new(CardId::new(100), SECRET_WEAPON_ID)];
    let before = serde_json::to_value(&r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let action = CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    };
    let opened = apply_combat_action_on_run(&r, action).unwrap();
    opened.validate().unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&opened).unwrap(),
        serde_json::to_value(apply_combat_action_on_run(&restored, action).unwrap()).unwrap()
    );
    println!(
        "energy_potion_transition={}",
        serde_json::json!({"initial":before,"combat_action":action,"result":opened})
    );
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.energy, 3);
    let next = step(&pending, Some(RunAction::ChooseDrawSelect { index: 0 }));
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn ordinary_energy_control() {
    let next = drink(&setup(vec![], vec![Potion::Energy]));
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 5);
}
#[test]
fn bark_energy_control() {
    let next = drink(&setup(vec![Relic::SacredBark], vec![Potion::Energy]));
    assert_eq!(next.combat.as_ref().unwrap().player.energy, 7);
}
#[test]
fn malformed_overflow_keeps_energy_error_and_immutable_input() {
    let mut r = setup(vec![], vec![Potion::Energy]);
    r.combat.as_mut().unwrap().player.energy = i32::MAX;
    let before = serde_json::to_value(&r).unwrap();
    assert!(matches!(
        apply_run_action(
            &r,
            RunAction::UsePotion {
                slot: 0,
                target: None
            }
        ),
        Err(sts_core::SimError::InvalidState(
            "Energy Potion energy gain overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(r).unwrap(), before);
}
