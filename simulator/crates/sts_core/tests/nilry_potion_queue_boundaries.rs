//! Additional source-backed queue boundaries; hook tests are explicitly marked.
use sts_core::adapter_internals::{
    apply_combat_action_on_run, apply_run_action, CardId, CardInstance, CombatAction, Potion,
    Relic, RunAction, RunState,
};
use sts_core::content::cards::STRIKE_R_ID;
fn setup(potions: Vec<Potion>) -> RunState {
    let mut r = RunState::combat_fixture_with_relics(vec![Relic::NilrysCodex, Relic::RunicPyramid]);
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
    next
}
fn drink(r: &RunState) -> RunState {
    let slot = (0..r.potion_capacity())
        .find(|s| r.potion_at_slot(*s).is_some())
        .unwrap();
    step(r, Some(RunAction::UsePotion { slot, target: None }))
}
#[test]
fn multiple_draw_potions_preserve_fifo_cost_timing() {
    for snecko_first in [false, true] {
        let potions = if snecko_first {
            vec![Potion::SneckoOil, Potion::Swift]
        } else {
            vec![Potion::Swift, Potion::SneckoOil]
        };
        let opened = step(&setup(potions), None);
        let first = drink(&opened);
        let pending = drink(&first);
        assert_eq!(
            opened.combat.as_ref().unwrap().piles,
            pending.combat.as_ref().unwrap().piles
        );
        let next = step(&pending, Some(RunAction::SkipCombatCardReward));
        let c = next.combat.as_ref().unwrap();
        let randomized = if snecko_first { 8 } else { 10 };
        assert_eq!(c.piles.hand.len(), 10);
        assert!(c.piles.hand[..randomized]
            .iter()
            .all(|card| card.temp_cost.is_some()));
        assert!(c.piles.hand[randomized..]
            .iter()
            .all(|card| card.temp_cost.is_none()));
        assert_eq!(
            next.card_random_rng_counter,
            opened.card_random_rng_counter + randomized as u32
        );
        assert!(c.pending_nilrys_codex_potion_actions.is_empty());
    }
}
#[test]
fn no_draw_removal_after_combust_precedes_later_potion_draws() {
    let mut r = setup(vec![Potion::Swift]);
    let c = r.combat.as_mut().unwrap();
    c.player.cannot_draw = true;
    c.player.no_draw_precedes_combust = false;
    c.player.powers.combust = 1;
    c.player.powers.combust_damage = 5;
    c.monsters[0].hp = 500;
    c.monsters[0].max_hp = 500;
    let opened = step(&r, None);
    let pending = drink(&opened);
    let next = step(&pending, Some(RunAction::SkipCombatCardReward));
    let c = next.combat.as_ref().unwrap();
    assert_eq!(c.piles.hand.len(), 10);
    assert!(!c.player.cannot_draw);
    assert_eq!(c.monsters[0].hp, 495);
}
#[test]
fn lethal_end_power_abandons_draw_and_cost_actions() {
    let mut r = setup(vec![Potion::SneckoOil]);
    let c = r.combat.as_mut().unwrap();
    c.player.powers.combust = 1;
    c.player.powers.combust_damage = 5;
    c.monsters[0].hp = 5;
    c.monsters[0].max_hp = 5;
    let opened = step(&r, None);
    let pending = drink(&opened);
    let next = step(&pending, Some(RunAction::SkipCombatCardReward));
    assert!(next.combat.is_none());
    assert_eq!(next.phase, sts_core::adapter_internals::RunPhase::Reward);
    assert_eq!(next.card_random_rng_counter, opened.card_random_rng_counter);
}
#[test]
fn lethal_end_power_retains_potion_use_heal_before_victory_heal() {
    let mut r = RunState::combat_fixture_with_relics(vec![
        Relic::NilrysCodex,
        Relic::ToyOrnithopter,
        Relic::BurningBlood,
    ]);
    r.potions = vec![Potion::Swift];
    let c = r.combat.as_mut().unwrap();
    c.player.hp = 30;
    c.player.powers.combust = 1;
    c.player.powers.combust_damage = 5;
    c.monsters[0].hp = 5;
    let opened = step(&r, None);
    let pending = drink(&opened);
    let next = step(&pending, Some(RunAction::SkipCombatCardReward));
    assert_eq!(next.phase, sts_core::adapter_internals::RunPhase::Reward);
    assert_eq!(
        next.hp, 40,
        "Combust loss, retained potion heal, then Burning Blood"
    );
    assert_eq!(next.card_random_rng_counter, opened.card_random_rng_counter);
}

#[test]
fn parked_choice_hook_preserves_serialized_pending_potions() {
    // Infrastructure diagnostic using the explicit publication hook, not a
    // claim that this direct hook is an accepted RunAction gameplay prefix.
    let opened = step(&setup(vec![Potion::Swift]), None);
    let mut pending = drink(&opened);
    let c = pending.combat.as_mut().unwrap();
    let actions = c.pending_nilrys_codex_potion_actions.clone();
    assert!(!actions.is_empty());
    sts_core::relic::nilrys_codex_park_choice_for_deferred_draw_insert(c, 0).unwrap();
    assert_eq!(c.pending_nilrys_codex_potion_actions, actions);
    let serialized = serde_json::to_value(&pending).unwrap();
    let restored: RunState = serde_json::from_value(serialized.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), serialized);
}
