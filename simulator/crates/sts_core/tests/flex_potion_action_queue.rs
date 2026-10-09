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
        "flex_queue_transition={}",
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
fn flex_gain_and_loss_wait_for_offer() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Flex]));
    let pending = drink(&opened);
    let p = &pending.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 0);
    assert_eq!(p.temp_strength, 0);
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 5);
    assert_eq!(p.temp_strength, 5);
}
#[test]
fn flex_use_heal_waits_for_offer() {
    let mut r = setup(
        vec![Relic::ToyOrnithopter],
        vec![Potion::Attack, Potion::Flex],
    );
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
    let n = choose(&pending);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 40);
}
#[test]
fn flex_ancient_order_uses_artifact_at_execution() {
    let opened = drink(&setup(
        vec![],
        vec![Potion::Attack, Potion::Ancient, Potion::Flex],
    ));
    let pending = drink(&drink(&opened));
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 5);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn flex_during_nilry_survives_next_start_until_next_end() {
    let opened = step(&setup(vec![Relic::NilrysCodex], vec![Potion::Flex]), None);
    let pending = drink(&opened);
    let n = step(&pending, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 5);
    assert_eq!(p.temp_strength, 5);
    let second = step(&n, None);
    let next = step(&second, Some(RunAction::SkipCombatCardReward));
    let p = &next.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 0);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn reverse_flex_then_ancient_keeps_loss_until_end() {
    let opened = drink(&setup(
        vec![],
        vec![Potion::Attack, Potion::Flex, Potion::Ancient],
    ));
    let pending = drink(&drink(&opened));
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 0);
    assert_eq!(p.temp_strength, 5);
    assert_eq!(p.powers.artifact, 1);
    let end = step(&n, None);
    let p = &end.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 5);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn repeated_bark_flex_caps_actual_keeps_full_nominal_debt() {
    let mut r = setup(
        vec![Relic::SacredBark],
        vec![Potion::Attack, Potion::Flex, Potion::Flex],
    );
    r.combat.as_mut().unwrap().player.powers.strength = 995;
    let opened = drink(&r);
    let pending = drink(&drink(&opened));
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 999);
    assert_eq!(p.temp_strength, 20);
    assert_eq!(p.powers.strength, 979);
    let end = step(&n, None);
    let p = &end.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 979);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn artifact_rejects_new_debt_before_debt_overflow() {
    let mut r = setup(vec![], vec![Potion::Flex]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.temp_strength = i32::MAX;
    p.powers.strength = -i32::MAX;
    p.powers.artifact = 1;
    let n = drink(&r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 5);
    assert_eq!(p.temp_strength, i32::MAX);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn queued_debt_overflow_rolls_back_positive_gain_and_whole_retrieval() {
    let mut r = setup(vec![], vec![Potion::Attack, Potion::Flex]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.temp_strength = i32::MAX;
    p.powers.strength = -i32::MAX;
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
fn flex_waits_for_secret_weapon_grid() {
    let mut r = setup(vec![], vec![Potion::Flex]);
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
    assert_eq!(pending.combat.as_ref().unwrap().player.temp_strength, 0);
    let n = step(&pending, Some(RunAction::ChooseDrawSelect { index: 0 }));
    assert_eq!(n.combat.as_ref().unwrap().player.temp_strength, 5);
}
#[test]
fn flex_waits_for_elixir_confirmation() {
    let opened = drink(&setup(vec![], vec![Potion::Elixir, Potion::Flex]));
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.temp_strength, 0);
    let selected = step(&pending, Some(RunAction::ChooseExhaustSelect { index: 0 }));
    let n = step(&selected, Some(RunAction::ConfirmExhaustSelect));
    assert_eq!(n.combat.as_ref().unwrap().player.temp_strength, 5);
}
#[test]
fn flex_survives_start_after_second_reward_in_nilry_continuation() {
    let opened = step(
        &setup(vec![Relic::NilrysCodex], vec![Potion::Attack, Potion::Flex]),
        None,
    );
    let pending = drink(&opened);
    let reward = step(&pending, Some(RunAction::SkipCombatCardReward));
    let pending = drink(&reward);
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 5);
    assert_eq!(p.temp_strength, 5);
    let second = step(&n, None);
    let n = step(&second, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 0);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn parked_flex_preserves_rng() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Flex]));
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
fn immediate_flex_control() {
    let n = drink(&setup(vec![], vec![Potion::Flex]));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength + p.temp_strength, 5);
    assert_eq!(p.temp_strength, 5);
}
