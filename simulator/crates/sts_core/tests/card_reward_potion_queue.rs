//! Reduced synthetic prefixes: source-backed queue timing, not dedicated trace parity.
use sts_core::adapter_internals::{
    apply_run_action, CardId, CardInstance, Potion, Relic, RunAction, RunState,
};
use sts_core::content::cards::STRIKE_R_ID;

fn setup(relics: Vec<Relic>, hand: u64, potions: Vec<Potion>) -> RunState {
    let mut run = RunState::combat_fixture_with_relics(relics);
    run.potions = potions;
    let c = run.combat.as_mut().unwrap();
    c.piles.hand = (100..100 + hand)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.draw_pile = (200..220)
        .map(|id| CardInstance::new(CardId::new(id), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    run.validate().unwrap();
    run
}

fn step(run: &RunState, action: RunAction) -> RunState {
    let before = serde_json::to_value(run).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    let next = apply_run_action(run, action).unwrap();
    assert_eq!(serde_json::to_value(run).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(apply_run_action(&restored, action).unwrap()).unwrap()
    );
    next.validate().unwrap();
    println!(
        "card_reward_queue_transition={}",
        serde_json::json!({"initial":before,"action":action,"result":next})
    );
    next
}
fn drink(run: &RunState) -> RunState {
    let slot = (0..run.potion_capacity())
        .find(|slot| run.potion_at_slot(*slot).is_some())
        .unwrap();
    step(run, RunAction::UsePotion { slot, target: None })
}
fn choose(run: &RunState) -> RunState {
    step(run, RunAction::ChooseCombatCardReward { index: 0 })
}
fn potion_offer(hand: u64, potion: Potion) -> RunState {
    drink(&setup(vec![], hand, vec![Potion::Attack, potion]))
}
fn assert_paused(before: &RunState, after: &RunState) {
    let a = before.combat.as_ref().unwrap();
    let b = after.combat.as_ref().unwrap();
    assert_eq!(
        b.piles, a.piles,
        "queued potion must not draw or randomize while the reward is open"
    );
    assert_eq!(b.rng, a.rng, "no potion RNG before selection settlement");
    assert_eq!(
        after.card_random_rng_counter,
        before.card_random_rng_counter
    );
}

#[test]
fn potion_reward_swift_waits_and_selected_card_takes_last_hand_slot() {
    let opened = potion_offer(9, Potion::Swift);
    let pending = drink(&opened);
    assert_paused(&opened, &pending);
    let selected = choose(&pending);
    let c = selected.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 10);
    assert_eq!(c.piles.draw_pile.len(), 20);
    assert!(c.piles.hand.last().unwrap().combat_only);
    assert!(c.piles.discard_pile.is_empty());
}
#[test]
fn potion_reward_snecko_waits_and_randomizes_selected_card_too() {
    let opened = potion_offer(6, Potion::SneckoOil);
    let pending = drink(&opened);
    assert_paused(&opened, &pending);
    let selected = choose(&pending);
    let c = selected.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 10);
    assert!(c.piles.hand.iter().all(|card| card.temp_cost.is_some()));
    assert_eq!(
        selected.card_random_rng_counter,
        opened.card_random_rng_counter + 10
    );
    assert_eq!(
        selected.card_random_rng_counter,
        c.rng.card_random_rng.counter()
    );
}
#[test]
fn skipping_potion_reward_flushes_draw_once() {
    let opened = potion_offer(7, Potion::Swift);
    let pending = drink(&opened);
    assert_paused(&opened, &pending);
    let skipped = step(&pending, RunAction::SkipCombatCardReward);
    assert_eq!(skipped.combat.as_ref().unwrap().piles.hand.len(), 10);
    assert_eq!(skipped.combat.as_ref().unwrap().piles.draw_pile.len(), 17);
}
#[test]
fn toolbox_swift_waits_for_selection_and_opening_draw() {
    let opened = setup(vec![Relic::Toolbox], 0, vec![Potion::Swift]);
    assert!(opened
        .combat
        .as_ref()
        .unwrap()
        .toolbox_card_reward_choices()
        .is_some());
    let pending = drink(&opened);
    assert_paused(&opened, &pending);
    let selected = choose(&pending);
    let c = selected.combat.as_ref().unwrap();
    assert_eq!(
        c.piles.hand.len(),
        7,
        "chosen card + three fixture opening cards + three Swift cards"
    );
    assert!(c.piles.hand[0].combat_only);
}
// Nilry's two confirmed failures remain preserved in the frozen eight-test
// baseline. Its paused end-turn powers/orb queue requires a separate fix.
#[test]
fn two_queued_potions_preserve_draw_then_randomization_order() {
    let opened = drink(&setup(
        vec![],
        3,
        vec![Potion::Attack, Potion::Swift, Potion::SneckoOil],
    ));
    let swift = drink(&opened);
    let snecko = drink(&swift);
    assert_paused(&opened, &snecko);
    let selected = choose(&snecko);
    let c = selected.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 10);
    assert!(c.piles.hand.iter().all(|card| card.temp_cost.is_some()));
    assert_eq!(
        selected.card_random_rng_counter,
        opened.card_random_rng_counter + 10
    );
}
#[test]
fn toolbox_snecko_uses_combat_rng_after_opening_draw() {
    let opened = setup(vec![Relic::Toolbox], 0, vec![Potion::SneckoOil]);
    let counter = opened
        .combat
        .as_ref()
        .unwrap()
        .rng
        .card_random_rng
        .counter();
    let pending = drink(&opened);
    assert_paused(&opened, &pending);
    let selected = choose(&pending);
    let c = selected.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 9);
    assert!(c.piles.hand.iter().all(|card| card.temp_cost.is_some()));
    assert_eq!(selected.card_random_rng_counter, counter + 9);
    assert_eq!(
        selected.card_random_rng_counter,
        c.rng.card_random_rng.counter()
    );
}

#[test]
fn draw_after_second_reward_waits_for_that_reward() {
    let first = drink(&setup(
        vec![],
        3,
        vec![Potion::Attack, Potion::Skill, Potion::Swift],
    ));
    let second = drink(&first);
    let pending = drink(&second);
    assert_paused(&second, &pending);
    let selected_first = choose(&pending);
    assert_eq!(selected_first.combat.as_ref().unwrap().piles.hand.len(), 4);
    let selected_second = choose(&selected_first);
    assert_eq!(selected_second.combat.as_ref().unwrap().piles.hand.len(), 8);
}

#[test]
fn draw_before_second_reward_drains_before_that_reward() {
    let first = drink(&setup(
        vec![],
        3,
        vec![Potion::Attack, Potion::Swift, Potion::Skill],
    ));
    let pending = drink(&first);
    let second = drink(&pending);
    let selected_first = choose(&second);
    assert_eq!(selected_first.combat.as_ref().unwrap().piles.hand.len(), 7);
    let selected_second = choose(&selected_first);
    assert_eq!(selected_second.combat.as_ref().unwrap().piles.hand.len(), 8);
}

#[test]
fn swift_without_a_reward_is_still_immediate() {
    let run = setup(vec![], 5, vec![Potion::Swift]);
    let next = drink(&run);
    assert_eq!(next.combat.as_ref().unwrap().piles.hand.len(), 8);
    assert_eq!(next.combat.as_ref().unwrap().piles.draw_pile.len(), 17);
}
