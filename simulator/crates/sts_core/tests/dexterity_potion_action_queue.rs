//! Source-backed synthetic legal gameplay prefixes, not interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, legal_run_decision_actions, CardId, CardInstance,
    CombatAction, Potion, Relic, RunAction, RunDecisionAction, RunState,
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
    let choice = match a {
        Some(a) => RunDecisionAction::Run(a),
        None => RunDecisionAction::Combat(CombatAction::EndTurn),
    };
    assert!(legal_run_decision_actions(r).unwrap().contains(&choice));
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
        "dexterity_queue_transition={}",
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
fn dexterity_waits_for_offer() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Dexterity]));
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let n = choose(&pending);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 2);
}
#[test]
fn dexterity_use_heal_waits_for_offer() {
    let mut r = setup(
        vec![Relic::ToyOrnithopter],
        vec![Potion::Attack, Potion::Dexterity],
    );
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
    let n = choose(&pending);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 40);
}
#[test]
fn dexterity_waits_for_old_nilry_loss_before_live_gain() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![Potion::Dexterity]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.dexterity = 998;
    p.temp_dexterity = 5;
    let opened = step(&r, None);
    let pending = drink(&opened);
    assert_eq!(
        pending.combat.as_ref().unwrap().player.powers.dexterity,
        998
    );
    let n = step(&pending, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 995);
    assert_eq!(p.temp_dexterity, 0);
}
#[test]
fn repeated_bark_caps_actual_preserves_existing_debt_and_artifact() {
    let mut r = setup(
        vec![Relic::SacredBark],
        vec![Potion::Attack, Potion::Dexterity, Potion::Dexterity],
    );
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.dexterity = 998;
    p.temp_dexterity = 20;
    p.powers.artifact = 2;
    let opened = drink(&r);
    let pending = drink(&drink(&opened));
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 999);
    assert_eq!(p.temp_dexterity, 20);
    assert_eq!(p.powers.artifact, 2);
}
#[test]
fn negative_dexterity_gain_retains_artifact() {
    let mut r = setup(vec![], vec![Potion::Dexterity]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.dexterity = -999;
    p.powers.artifact = 1;
    let n = drink(&r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, -997);
    assert_eq!(p.powers.artifact, 1);
}
#[test]
fn dexterity_waits_for_elixir() {
    let opened = drink(&setup(vec![], vec![Potion::Elixir, Potion::Dexterity]));
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let selected = step(&pending, Some(RunAction::ChooseExhaustSelect { index: 0 }));
    let n = step(&selected, Some(RunAction::ConfirmExhaustSelect));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 2);
}
#[test]
fn dexterity_waits_for_secret_weapon_grid() {
    let mut r = setup(vec![], vec![Potion::Dexterity]);
    r.combat.as_mut().unwrap().piles.hand =
        vec![CardInstance::new(CardId::new(100), SECRET_WEAPON_ID)];
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    });
    assert!(legal_run_decision_actions(&r).unwrap().contains(&a));
    let before = serde_json::to_value(&r).unwrap();
    let opened = sts_core::adapter_internals::apply_run_decision_action(&r, a).unwrap();
    opened.validate().unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    let restored: RunState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(
            sts_core::adapter_internals::apply_run_decision_action(&restored, a).unwrap()
        )
        .unwrap(),
        serde_json::to_value(&opened).unwrap()
    );
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let n = step(&pending, Some(RunAction::ChooseDrawSelect { index: 0 }));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 2);
}
#[test]
fn parked_dexterity_preserves_rng() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Dexterity]));
    let before = serde_json::to_value(&opened.combat.as_ref().unwrap().rng).unwrap();
    let counter = opened.card_random_rng_counter;
    let pending = drink(&opened);
    assert_eq!(
        serde_json::to_value(&pending.combat.as_ref().unwrap().rng).unwrap(),
        before
    );
    assert_eq!(pending.card_random_rng_counter, counter);
}
#[test]
fn immediate_dexterity_overflow_is_atomic() {
    let mut r = setup(vec![], vec![Potion::Dexterity]);
    r.combat.as_mut().unwrap().player.powers.dexterity = i32::MAX;
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
            "combat potion stat gain overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
}
#[test]
fn deferred_dexterity_overflow_is_atomic() {
    let mut r = setup(vec![], vec![Potion::Attack, Potion::Dexterity]);
    r.combat.as_mut().unwrap().player.powers.dexterity = i32::MAX;
    let opened = drink(&r);
    let pending = drink(&opened);
    let before = serde_json::to_value(&pending).unwrap();
    assert!(matches!(
        apply_run_action(&pending, RunAction::ChooseCombatCardReward { index: 0 }),
        Err(sts_core::SimError::InvalidState(
            "combat potion stat gain overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(&pending).unwrap(), before);
}
#[test]
fn immediate_dexterity_control() {
    let n = drink(&setup(vec![], vec![Potion::Dexterity]));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 2);
}
