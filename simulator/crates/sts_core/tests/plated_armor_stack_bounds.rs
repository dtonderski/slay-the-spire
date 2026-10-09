//! Source-backed synthetic legal gameplay prefixes, not interaction-trace parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, legal_run_decision_actions, CardId, CardInstance,
    CombatAction, Potion, Relic, RunAction, RunDecisionAction, RunState,
};
use sts_core::content::cards::STRIKE_R_ID;
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
        "steel_queue_transition={}",
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
fn existing_near_cap_bark_stack_is_bounded() {
    let mut r = setup(vec![Relic::SacredBark], vec![Potion::EssenceOfSteel]);
    r.combat.as_mut().unwrap().player.powers.plated_armor = 995;
    let n = drink(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 999);
}
#[test]
fn existing_cap_normal_stack_is_bounded() {
    let mut r = setup(vec![], vec![Potion::EssenceOfSteel]);
    r.combat.as_mut().unwrap().player.powers.plated_armor = 999;
    let n = drink(&r);
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 999);
}
#[test]
fn queued_repeated_bark_reads_existing_stack_not_constructor_amount() {
    let mut r = setup(
        vec![Relic::SacredBark],
        vec![
            Potion::Attack,
            Potion::EssenceOfSteel,
            Potion::EssenceOfSteel,
        ],
    );
    let p = &mut r.combat.as_mut().unwrap().player;
    p.powers.plated_armor = 995;
    p.powers.artifact = 1;
    let opened = drink(&r);
    let pending = drink(&drink(&opened));
    let n = choose(&pending);
    let p = &n.combat.as_ref().unwrap().player;
    assert_eq!(p.powers.plated_armor, 999);
    assert_eq!(p.powers.artifact, 1);
}
#[test]
fn raw_small_constructor_control() {
    let n = drink(&setup(vec![], vec![Potion::EssenceOfSteel]));
    assert_eq!(n.combat.as_ref().unwrap().player.powers.plated_armor, 4);
}
