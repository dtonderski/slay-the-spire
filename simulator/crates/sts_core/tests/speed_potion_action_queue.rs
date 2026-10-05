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
        "speed_queue_transition={}",
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
fn speed_waits_behind_potion_reward() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Speed]));
    let pending = drink(&opened);
    let p = &pending.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 0);
    assert_eq!(p.temp_dexterity, 0);
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.temp_dexterity, 5);
}
#[test]
fn speed_waits_behind_nilry_until_after_end_power_window() {
    let opened = step(&setup(vec![Relic::NilrysCodex], vec![Potion::Speed]), None);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let n = step(&pending, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.temp_dexterity, 5);
    assert_eq!(n.combat.as_ref().unwrap().monsters[0].moves_executed, 1);
}
#[test]
fn speed_use_heal_waits_behind_same_reward() {
    let mut r = setup(
        vec![Relic::ToyOrnithopter],
        vec![Potion::Attack, Potion::Speed],
    );
    r.combat.as_mut().unwrap().player.hp = 30;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.hp, 30);
    let n = choose(&pending);
    assert_eq!(n.combat.as_ref().unwrap().player.hp, 40);
}
#[test]
fn speed_does_not_consume_artifact_before_reward_closes() {
    let mut r = setup(vec![], vec![Potion::Attack, Potion::Speed]);
    r.combat.as_mut().unwrap().player.powers.artifact = 1;
    let opened = drink(&r);
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.artifact, 1);
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.temp_dexterity, 0);
}
#[test]
fn speed_waits_for_toolbox_opening_draw_then_applies_bark_amount() {
    let pending = drink(&setup(
        vec![Relic::Toolbox, Relic::SacredBark],
        vec![Potion::Speed],
    ));
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 10);
    assert_eq!(p.temp_dexterity, 10);
}
#[test]
fn speed_waits_for_elixir_confirmation() {
    let opened = drink(&setup(vec![], vec![Potion::Elixir, Potion::Speed]));
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let selected = step(&pending, Some(RunAction::ChooseExhaustSelect { index: 0 }));
    let n = step(&selected, Some(RunAction::ConfirmExhaustSelect));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
}
#[test]
fn speed_waits_for_secret_weapon_grid() {
    let mut r = setup(vec![], vec![Potion::Speed]);
    r.combat.as_mut().unwrap().piles.hand =
        vec![CardInstance::new(CardId::new(100), SECRET_WEAPON_ID)];
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(100),
        target: None,
    });
    assert!(legal_run_decision_actions(&r).unwrap().contains(&a));
    let opened = sts_core::adapter_internals::apply_run_decision_action(&r, a).unwrap();
    opened.validate().unwrap();
    let pending = drink(&opened);
    assert_eq!(pending.combat.as_ref().unwrap().player.powers.dexterity, 0);
    let n = step(&pending, Some(RunAction::ChooseDrawSelect { index: 0 }));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
}
#[test]
fn multiple_speed_pairs_preserve_old_debt_and_full_nominal_caps() {
    let mut r = setup(vec![], vec![Potion::Attack, Potion::Speed, Potion::Speed]);
    r.combat.as_mut().unwrap().player.powers.dexterity = 997;
    r.combat.as_mut().unwrap().player.temp_dexterity = 3;
    let opened = drink(&r);
    let pending = drink(&drink(&opened));
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 999);
    assert_eq!(p.temp_dexterity, 13);
}
#[test]
fn speed_after_nilry_power_window_expires_at_next_end_not_start() {
    let opened = step(&setup(vec![Relic::NilrysCodex], vec![Potion::Speed]), None);
    let pending = drink(&opened);
    let n = step(&pending, Some(RunAction::SkipCombatCardReward));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 5);
    let n = step(&n, None);
    let n = step(&n, Some(RunAction::SkipCombatCardReward));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.dexterity, 0);
    assert_eq!(n.combat.as_ref().unwrap().player.temp_dexterity, 0);
}
#[test]
fn queue_preserves_offer_rng_until_actual_settlement() {
    let opened = drink(&setup(vec![], vec![Potion::Attack, Potion::Speed]));
    let before = opened.card_random_rng_counter;
    let c = serde_json::to_value(&opened.combat.as_ref().unwrap().rng).unwrap();
    let pending = drink(&opened);
    assert_eq!(pending.card_random_rng_counter, before);
    assert_eq!(
        serde_json::to_value(&pending.combat.as_ref().unwrap().rng).unwrap(),
        c
    );
}
#[test]
fn immediate_debt_overflow_rolls_back_prior_positive_gain_and_consumption() {
    let mut r = setup(vec![], vec![Potion::Speed]);
    r.combat.as_mut().unwrap().player.temp_dexterity = i32::MAX;
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
fn artifact_rejects_only_new_debt_before_malformed_debt_arithmetic() {
    let mut r = setup(vec![], vec![Potion::Speed]);
    r.combat.as_mut().unwrap().player.temp_dexterity = i32::MAX;
    r.combat.as_mut().unwrap().player.powers.artifact = 1;
    let n = drink(&r);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.temp_dexterity, i32::MAX);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn ordinary_speed_control() {
    let n = drink(&setup(vec![], vec![Potion::Speed]));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.dexterity, 5);
    assert_eq!(p.temp_dexterity, 5);
}
