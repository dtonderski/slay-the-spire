//! Source-backed synthetic queue boundaries, not dedicated real-game parity.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, Potion,
    Relic, RunAction, RunState,
};
use sts_core::content::cards::STRIKE_R_ID;
fn setup(hand: u64, potions: Vec<Potion>, pyramid: bool) -> RunState {
    let mut relics = vec![Relic::NilrysCodex];
    if pyramid {
        relics.push(Relic::RunicPyramid);
    }
    let mut r = RunState::combat_fixture_with_relics(relics);
    r.potions = potions;
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = (100..100 + hand)
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
fn action(r: &RunState, a: Option<RunAction>) -> RunState {
    let before = serde_json::to_value(r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let execute = |s: &RunState| match a {
        Some(a) => apply_run_action(s, a),
        None => apply_combat_action_on_run(s, CombatAction::EndTurn),
    };
    let next = execute(r).unwrap();
    let repeat = execute(&restored).unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(repeat).unwrap()
    );
    next.validate().unwrap();
    println!(
        "nilry_potion_transition={}",
        serde_json::json!({"initial":before,"run_action":a,"combat_action":a.is_none().then_some(CombatAction::EndTurn),"result":next})
    );
    next
}
fn drink(r: &RunState) -> RunState {
    let slot = (0..r.potion_capacity())
        .find(|s| r.potion_at_slot(*s).is_some())
        .unwrap();
    action(r, Some(RunAction::UsePotion { slot, target: None }))
}
fn choose(r: &RunState) -> RunState {
    action(r, Some(RunAction::ChooseCombatCardReward { index: 0 }))
}
fn skip(r: &RunState) -> RunState {
    action(r, Some(RunAction::SkipCombatCardReward))
}
fn paused(a: &RunState, b: &RunState) {
    assert_eq!(
        a.combat.as_ref().unwrap().piles,
        b.combat.as_ref().unwrap().piles,
        "potion must wait behind CodexAction"
    );
    assert_eq!(
        a.combat.as_ref().unwrap().rng,
        b.combat.as_ref().unwrap().rng
    );
    assert_eq!(a.card_random_rng_counter, b.card_random_rng_counter);
}
#[test]
fn swift_does_not_mutate_open_codex_offer() {
    let opened = action(&setup(9, vec![Potion::Swift], false), None);
    let pending = drink(&opened);
    paused(&opened, &pending);
    let selected = choose(&pending);
    assert!(selected
        .combat
        .as_ref()
        .unwrap()
        .combat_card_reward_choices()
        .is_none());
}
#[test]
fn snecko_waits_and_run_counter_matches_combat_after_skip() {
    let opened = action(&setup(7, vec![Potion::SneckoOil], false), None);
    let pending = drink(&opened);
    paused(&opened, &pending);
    let next = skip(&pending);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(
        next.card_random_rng_counter,
        opened.card_random_rng_counter + 10
    );
    assert_eq!(
        next.card_random_rng_counter,
        c.rng.card_random_rng.counter()
    );
}
#[test]
fn swift_skip_draws_before_discard_and_next_hand() {
    let opened = action(&setup(7, vec![Potion::Swift], false), None);
    let pending = drink(&opened);
    paused(&opened, &pending);
    let next = skip(&pending);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.discard_pile.len(), 10);
    assert_eq!(c.piles.hand.len(), 5);
    assert_eq!(c.piles.hand[0].id, CardId::new(216));
    assert!(c
        .piles
        .discard_pile
        .iter()
        .any(|card| card.id == CardId::new(219)));
}
#[test]
fn snecko_randomizes_retained_hand_before_next_turn_refill() {
    let opened = action(&setup(3, vec![Potion::SneckoOil], true), None);
    let pending = drink(&opened);
    paused(&opened, &pending);
    let next = skip(&pending);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 10);
    assert!(c.piles.hand[..8]
        .iter()
        .all(|card| card.temp_cost.is_some()));
    assert!(c.piles.hand[8..]
        .iter()
        .all(|card| card.temp_cost.is_none()));
    assert_eq!(
        next.card_random_rng_counter,
        opened.card_random_rng_counter + 8
    );
}
#[test]
fn original_power_queue_removes_no_draw_before_queued_swift() {
    let mut r = setup(7, vec![Potion::Swift], false);
    let c = r.combat.as_mut().unwrap();
    c.player.cannot_draw = true;
    c.player.no_draw_precedes_combust = true;
    c.player.powers.combust = 1;
    c.player.powers.combust_damage = 5;
    c.monsters[0].hp = 500;
    c.monsters[0].max_hp = 500;
    let opened = action(&r, None);
    let pending = drink(&opened);
    paused(&opened, &pending);
    let next = skip(&pending);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.discard_pile.len(), 10);
    assert_eq!(c.monsters[0].hp, 495);
    assert!(!c.player.cannot_draw);
}
#[test]
fn no_potion_control_still_resumes_once() {
    let opened = action(&setup(7, vec![], false), None);
    let next = skip(&opened);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.discard_pile.len(), 7);
    assert_eq!(c.piles.hand.len(), 5);
    assert_eq!(c.monsters[0].moves_executed, 1);
}
#[test]
fn draw_potion_before_another_reward_still_waits_for_codex() {
    let opened = action(&setup(3, vec![Potion::Swift, Potion::Attack], false), None);
    let pending = drink(&opened);
    paused(&opened, &pending);
    let queued_reward = drink(&pending);
    let selected = skip(&queued_reward);
    let c = selected.combat.as_ref().unwrap();
    assert!(c.potion_card_reward_choices().is_some());
    assert_eq!(c.monsters[0].moves_executed, 0);
    assert_eq!(c.piles.hand.len(), 6);
    let next = choose(&selected);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.monsters[0].moves_executed, 1);
    assert_eq!(c.piles.discard_pile.len(), 7);
}
#[test]
fn draw_after_another_reward_waits_for_that_selection_then_resumes() {
    let opened = action(&setup(3, vec![Potion::Attack, Potion::Swift], false), None);
    let queued_reward = drink(&opened);
    let pending = drink(&queued_reward);
    paused(&queued_reward, &pending);
    let selected = skip(&pending);
    let c = selected.combat.as_ref().unwrap();
    assert!(c.potion_card_reward_choices().is_some());
    assert_eq!(c.monsters[0].moves_executed, 0);
    assert_eq!(c.piles.hand.len(), 3);
    let next = choose(&selected);
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.monsters[0].moves_executed, 1);
    assert_eq!(c.piles.discard_pile.len(), 7);
}
