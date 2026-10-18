//! Source-backed controls for ExhumeAction's nonmodal paths and UseCardAction
//! exhaust -> DeadBranch.onExhaust -> MakeTempCardInHandAction before DE draws.
//! Real regression: FIDL00293 step352 (and four other first boundaries).
use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{DEFEND_R_ID, EXHUME_ID, EXHUME_PLUS_ID, HAVOC_ID, STRIKE_R_ID},
        shop_pool::ironclad_combat_discovery_pool,
    },
    CardId, CardInstance, CombatAction, CombatState, Relic,
};
fn fixture(content: sts_core::adapter_internals::ContentId, target: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.piles.hand = vec![CardInstance::new(CardId::new(17), content)];
    s.piles.draw_pile = vec![CardInstance::new(CardId::new(90), DEFEND_R_ID)];
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile = if target {
        vec![CardInstance::new(CardId::new(33), STRIKE_R_ID)]
    } else {
        vec![]
    };
    s.player.authority.relics.push(Relic::DeadBranch);
    s
}
fn play(s: &CombatState) -> CombatState {
    apply_combat_action(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(17),
            target: None,
        },
    )
    .unwrap()
}
fn generated(s: &CombatState) -> sts_core::adapter_internals::ContentId {
    let mut rng = s.rng.card_random_rng.clone();
    let pool = ironclad_combat_discovery_pool();
    pool[rng.random_int((pool.len() - 1) as i32) as usize]
}
fn assert_one_branch_draw(s: &CombatState, n: &CombatState) {
    let mut expected = s.rng.card_random_rng.clone();
    expected.random_int((ironclad_combat_discovery_pool().len() - 1) as i32);
    assert_eq!(
        serde_json::to_value(&n.rng.card_random_rng).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
#[test]
fn singleton_return_then_source_exhaust_generates_one_branch_card() {
    for content in [EXHUME_ID, EXHUME_PLUS_ID] {
        let s = fixture(content, true);
        let n = play(&s);
        assert!(n.decision.is_none());
        assert_eq!(n.piles.hand.len(), 2);
        assert_eq!(n.piles.hand[0].id, CardId::new(33));
        assert_eq!(n.piles.hand[1].content_id, generated(&s));
        assert_eq!(n.piles.exhaust_pile.len(), 1);
        assert_eq!(n.piles.exhaust_pile[0].id, CardId::new(17));
        assert_one_branch_draw(&s, &n);
    }
}
#[test]
fn empty_or_only_exhumes_still_exhausts_source_and_triggers_branch() {
    for count in 0..3 {
        let mut s = fixture(EXHUME_ID, false);
        s.piles.exhaust_pile = (0..count)
            .map(|i| CardInstance::new(CardId::new(40 + i), EXHUME_ID))
            .collect();
        let n = play(&s);
        assert!(n.decision.is_none());
        assert_eq!(n.piles.hand.len(), 1);
        assert_eq!(n.piles.hand[0].content_id, generated(&s));
        assert_eq!(n.piles.exhaust_pile.len(), count as usize + 1);
        assert_one_branch_draw(&s, &n);
    }
}
#[test]
fn singleton_fills_hand_before_branch_overflow_goes_to_discard() {
    let mut s = fixture(EXHUME_ID, true);
    s.piles
        .hand
        .extend((40..49).map(|i| CardInstance::new(CardId::new(i), STRIKE_R_ID)));
    let n = play(&s);
    assert_eq!(n.piles.hand.len(), 10);
    assert_eq!(n.piles.hand[9].id, CardId::new(33));
    assert_eq!(n.piles.discard_pile.len(), 1);
    assert_eq!(n.piles.discard_pile[0].content_id, generated(&s));
    assert_one_branch_draw(&s, &n);
}
#[test]
fn branch_generation_precedes_dark_embrace_draw_at_hand_cap() {
    let mut s = fixture(EXHUME_ID, true);
    s.piles
        .hand
        .extend((40..48).map(|i| CardInstance::new(CardId::new(i), STRIKE_R_ID)));
    s.player.powers.dark_embrace = 1;
    s.player.powers.feel_no_pain = 3;
    let n = play(&s);
    assert_eq!(n.player.block, 3);
    assert_eq!(n.piles.hand.len(), 10);
    assert_eq!(n.piles.hand[9].content_id, generated(&s));
    assert!(n.piles.discard_pile.is_empty());
    assert_eq!(n.piles.draw_pile, s.piles.draw_pile);
    assert_one_branch_draw(&s, &n);
}
#[test]
fn copied_use_does_not_trigger_source_exhaust_a_second_time() {
    for extra in [0, 9] {
        let mut s = fixture(EXHUME_PLUS_ID, true);
        s.piles
            .hand
            .extend((40..40 + extra).map(|i| CardInstance::new(CardId::new(i), STRIKE_R_ID)));
        s.duplication_potion_pending = true;
        s.duplication_potion_stacks = 1;
        let n = play(&s);
        assert_eq!(n.piles.hand.len(), (extra + 2).min(10) as usize);
        assert_eq!(n.piles.exhaust_pile.len(), 1);
        assert_one_branch_draw(&s, &n);
    }
}
#[test]
fn havoc_autoplayed_exhume_with_empty_exhaust_generates_once() {
    let mut s = fixture(HAVOC_ID, false);
    s.piles.draw_pile[0].content_id = EXHUME_ID;
    // Havoc.use reserves a random monster before constructing PlayTopCardAction,
    // even when the selected card is a nontargeting skill and only one foe lives.
    let mut after_havoc_target_roll = s.clone();
    after_havoc_target_roll.rng.card_random_rng.random_int(0);
    let n = play(&s);
    assert!(n.decision.is_none());
    assert_eq!(n.piles.hand.len(), 1);
    assert_eq!(
        n.piles.hand[0].content_id,
        generated(&after_havoc_target_roll)
    );
    assert_eq!(n.piles.exhaust_pile.len(), 1);
    assert_eq!(n.piles.exhaust_pile[0].id, CardId::new(90));
    assert_eq!(n.piles.discard_pile[0].id, CardId::new(17));
    assert_one_branch_draw(&after_havoc_target_roll, &n);
}
#[test]
fn no_branch_still_has_power_callbacks_without_card_random_draw() {
    let mut s = fixture(EXHUME_ID, false);
    s.player.authority.relics.clear();
    s.player.powers.dark_embrace = 1;
    s.player.powers.feel_no_pain = 3;
    let n = play(&s);
    assert_eq!(n.player.block, 3);
    assert_eq!(n.piles.hand[0].id, CardId::new(90));
    assert_eq!(
        serde_json::to_value(n.rng.card_random_rng).unwrap(),
        serde_json::to_value(s.rng.card_random_rng).unwrap()
    );
}
