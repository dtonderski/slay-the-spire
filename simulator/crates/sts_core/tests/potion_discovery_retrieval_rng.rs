//! Same native DiscoveryAction duration loop as the already implemented card.
//! Source-derived collection.2 fixed60Hz profile, not an inferred frame count.
//! Primary real witness: QLFD00047 Potion176/CHOOSE177 -> Wild Strike183.
use sts_core::adapter_internals::{
    apply_run_action,
    content::{
        cards::STRIKE_R_ID,
        shop_pool::{colorless_discovery_card_choices, discovery_card_choices},
    },
    CardId, CardInstance, CardType, CombatState, ContentId, Potion, RunAction, RunState,
};
fn setup(potions: Vec<Potion>) -> RunState {
    let mut r = RunState::combat_fixture();
    r.potions = potions;
    r.empty_potion_slots = (r.potions.len()..3).collect();
    let c = r.combat.as_mut().unwrap();
    c.piles.hand = vec![CardInstance::new(CardId::new(100), STRIKE_R_ID)];
    c.piles.draw_pile = (200..220)
        .map(|i| CardInstance::new(CardId::new(i), STRIKE_R_ID))
        .collect();
    c.piles.discard_pile.clear();
    c.piles.exhaust_pile.clear();
    r.validate().unwrap();
    r
}
fn step(r: &RunState, a: RunAction) -> RunState {
    apply_run_action(r, a).unwrap()
}
fn drink(r: &RunState, slot: usize) -> RunState {
    step(r, RunAction::UsePotion { slot, target: None })
}
fn choices(s: &mut CombatState, p: Potion) -> Vec<ContentId> {
    match p {
        Potion::Attack => discovery_card_choices(&mut s.rng.card_random_rng, CardType::Attack, 3),
        Potion::Skill => discovery_card_choices(&mut s.rng.card_random_rng, CardType::Skill, 3),
        Potion::Power => discovery_card_choices(&mut s.rng.card_random_rng, CardType::Power, 3),
        Potion::Colorless => colorless_discovery_card_choices(&mut s.rng.card_random_rng, 3),
        _ => unreachable!(),
    }
}
fn finish(s: &mut CombatState, p: Potion) -> usize {
    // Mirror Java float subtraction of Settings.ACTION_DUR_FAST by the mod's
    // GAMEPLAY_DELTA. First/opening update already occurred; no outcome input.
    let delta = 1.0_f32 / 60.0_f32;
    let mut duration = 0.25_f32 - delta;
    let mut count = 0;
    while duration >= 0.0 {
        choices(s, p);
        count += 1;
        duration -= delta;
    }
    count
}
fn assert_rng(expected: &CombatState, r: &RunState) {
    let actual = r.combat.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&expected.rng.card_random_rng).unwrap(),
        serde_json::to_value(&actual.rng.card_random_rng).unwrap()
    );
    assert_eq!(
        r.card_random_rng_counter,
        actual.rng.card_random_rng.counter()
    );
}
#[test]
fn native_float_duration_derives_fifteen_post_opening_generations() {
    let mut s = setup(vec![Potion::Attack]).combat.unwrap();
    assert_eq!(finish(&mut s, Potion::Attack), 15);
}
#[test]
fn every_potion_kind_keeps_its_selected_offer_and_finishes_its_own_pool() {
    for p in [
        Potion::Attack,
        Potion::Skill,
        Potion::Power,
        Potion::Colorless,
    ] {
        let opened = drink(&setup(vec![p]), 0);
        let c = opened.combat.as_ref().unwrap();
        let chosen = c.potion_card_reward_choices().unwrap()[1].content_id;
        let mut expected = c.clone();
        finish(&mut expected, p);
        let n = step(&opened, RunAction::ChooseCombatCardReward { index: 1 });
        assert_eq!(
            n.combat
                .as_ref()
                .unwrap()
                .piles
                .hand
                .last()
                .unwrap()
                .content_id,
            chosen
        );
        assert_rng(&expected, &n);
    }
}
#[test]
fn skip_still_finishes_the_same_duration_generations() {
    for p in [
        Potion::Attack,
        Potion::Skill,
        Potion::Power,
        Potion::Colorless,
    ] {
        let opened = drink(&setup(vec![p]), 0);
        let c = opened.combat.as_ref().unwrap();
        let mut expected = c.clone();
        finish(&mut expected, p);
        let n = step(&opened, RunAction::SkipCombatCardReward);
        assert_eq!(n.combat.as_ref().unwrap().piles.hand, c.piles.hand);
        assert_rng(&expected, &n);
    }
}
#[test]
fn retrieval_generations_precede_queued_swift_confusion_draws() {
    let mut r = setup(vec![Potion::Skill, Potion::Swift]);
    r.combat.as_mut().unwrap().player.powers.confusion = 1;
    let opened = drink(&r, 0);
    let pending = drink(&opened, 1);
    assert_eq!(
        pending.combat.as_ref().unwrap().rng,
        opened.combat.as_ref().unwrap().rng
    );
    let mut expected = pending.combat.as_ref().unwrap().clone();
    finish(&mut expected, Potion::Skill);
    for _ in 0..3 {
        expected.rng.card_random_rng.random_int(3);
    }
    let n = step(&pending, RunAction::ChooseCombatCardReward { index: 0 });
    assert_rng(&expected, &n);
}
#[test]
fn earlier_retrieval_generations_precede_the_next_queued_potion_offer() {
    let opened = drink(&setup(vec![Potion::Attack, Potion::Power]), 0);
    let pending = drink(&opened, 1);
    let mut expected = pending.combat.as_ref().unwrap().clone();
    finish(&mut expected, Potion::Attack);
    let next_offer = choices(&mut expected, Potion::Power);
    let n = step(&pending, RunAction::ChooseCombatCardReward { index: 0 });
    assert_rng(&expected, &n);
    assert_eq!(
        n.combat
            .as_ref()
            .unwrap()
            .potion_card_reward_choices()
            .unwrap()
            .iter()
            .map(|c| c.content_id)
            .collect::<Vec<_>>(),
        next_offer
    );
}
#[test]
fn snapshot_restore_and_rejected_choice_leave_the_offer_and_rng_authoritative() {
    let r = drink(&setup(vec![Potion::Colorless]), 0);
    let before = serde_json::to_value(&r).unwrap();
    let restored: RunState = serde_json::from_value(before.clone()).unwrap();
    assert!(apply_run_action(&r, RunAction::ChooseCombatCardReward { index: 99 }).is_err());
    assert_eq!(serde_json::to_value(&r).unwrap(), before);
    assert_eq!(
        step(&r, RunAction::ChooseCombatCardReward { index: 0 }),
        step(&restored, RunAction::ChooseCombatCardReward { index: 0 })
    );
}
