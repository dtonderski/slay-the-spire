//! Source-backed synthetic legal gameplay prefixes, not interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, legal_run_decision_actions, CardId, CardInstance,
    CombatAction, Potion, Relic, RunAction, RunDecisionAction, RunState,
};
use sts_core::content::cards::{FLEX_ID, STRIKE_R_ID};
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
        "strength_queue_transition={}",
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
fn old_strength_loss_blocks_with_existing_artifact_without_a_potion() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.strength = 795;
    p.temp_strength = 5;
    p.powers.artifact = 1;
    let opened = step(&r, None);
    let n = step(&opened, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 800);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn flex_then_ancient_applies_old_loss_before_nilry_continues() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![Potion::Ancient]);
    r.combat
        .as_mut()
        .unwrap()
        .piles
        .hand
        .push(CardInstance::new(CardId::new(101), FLEX_ID));
    let a = RunDecisionAction::Combat(CombatAction::PlayCard {
        card_id: CardId::new(101),
        target: None,
    });
    assert!(legal_run_decision_actions(&r).unwrap().contains(&a));
    let before = serde_json::to_value(&r).unwrap();
    let played = sts_core::adapter_internals::apply_run_decision_action(&r, a).unwrap();
    played.validate().unwrap();
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    let restored: RunState = serde_json::from_value(before).unwrap();
    assert_eq!(
        serde_json::to_value(
            sts_core::adapter_internals::apply_run_decision_action(&restored, a).unwrap()
        )
        .unwrap(),
        serde_json::to_value(&played).unwrap()
    );
    eprintln!(
        "nilry_flex_transition={}",
        serde_json::json!({"initial":r,"action":"PlayFlex101","next":played})
    );
    let with_artifact = drink(&played);
    let opened = step(&with_artifact, None);
    let n = step(&opened, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 2);
    assert_eq!(p.powers.artifact, 0);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn zero_loss_control_keeps_artifact() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![]);
    r.combat.as_mut().unwrap().player.powers.artifact = 1;
    let opened = step(&r, None);
    let n = choose(&opened);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 0);
    assert_eq!(p.powers.artifact, 1);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn old_loss_settles_before_later_ancient_and_reward_barrier() {
    let mut r = setup(
        vec![Relic::NilrysCodex],
        vec![Potion::Ancient, Potion::Attack],
    );
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.strength = 3;
    p.temp_strength = 2;
    let opened = step(&r, None);
    let pending = drink(&drink(&opened));
    let reward = step(&pending, Some(RunAction::SkipCombatCardReward));
    let p = &reward.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 1);
    let n = choose(&reward);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 3);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 1);
}
#[test]
fn blocked_old_loss_only_runs_once_across_later_reward() {
    let mut r = setup(
        vec![Relic::NilrysCodex],
        vec![Potion::Attack, Potion::Strength],
    );
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.strength = 3;
    p.temp_strength = 2;
    p.powers.artifact = 1;
    let opened = step(&r, None);
    let pending = drink(&opened);
    let reward = step(&pending, Some(RunAction::SkipCombatCardReward));
    let p = &reward.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 5);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 0);
    let new = drink(&reward);
    let n = choose(&new);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 7);
    assert_eq!(p.temp_strength, 0);
    assert_eq!(p.powers.artifact, 0);
}
#[test]
fn choosing_nilry_settles_old_loss_without_next_start_repair() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![Potion::Strength]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.strength = 995;
    p.temp_strength = 4;
    p.powers.ritual = 1;
    let opened = step(&r, None);
    let pending = drink(&opened);
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 997);
    assert_eq!(p.temp_strength, 0);
}
#[test]
fn nilry_artifact_component_overflow_is_atomic() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.strength = i32::MAX;
    p.temp_strength = 1;
    p.powers.artifact = 1;
    let opened = step(&r, None);
    let before = serde_json::to_value(&opened).unwrap();
    assert!(matches!(
        apply_run_action(&opened, RunAction::SkipCombatCardReward),
        Err(sts_core::SimError::InvalidState(
            "combat integer addition overflows i32"
        ))
    ));
    assert_eq!(serde_json::to_value(&opened).unwrap(), before);
}
#[test]
fn strength_after_nilry_cap_and_expiry_window() {
    let mut r = setup(vec![Relic::NilrysCodex], vec![Potion::Strength]);
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.strength = 995;
    p.temp_strength = 4;
    p.powers.ritual = 1;
    let opened = step(&r, None);
    let pending = drink(&opened);
    let n = step(&pending, Some(RunAction::SkipCombatCardReward));
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.strength, 997);
    assert_eq!(p.temp_strength, 0);
}
