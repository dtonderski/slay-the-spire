//! PC DrawCardAction split -> EmptyDeckShuffleAction constructor onShuffle.
//! Synthetic controls supplement FIDL00144 step1278 and other natural cases.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::cards::{BATTLE_TRANCE_ID, DEFEND_R_ID, VOID_ID},
    CardId, CardInstance, CombatAction, CombatState, Relic,
};
fn fixture(cards: usize) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.piles.hand = vec![CardInstance::new(CardId::new(99), BATTLE_TRANCE_ID)];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    for i in 0..cards {
        s.piles
            .draw_pile
            .push(CardInstance::new(CardId::new(200 + i as u64), DEFEND_R_ID));
    }
    s.player.energy = 3;
    s
}
fn play(s: &CombatState) -> CombatState {
    apply_combat_action(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None,
        },
    )
    .unwrap()
}
#[test]
fn overdraw_empty_shuffle_advances_sundial_and_queues_its_energy() {
    let mut s = fixture(2);
    s.player.authority.relics.push(Relic::Sundial);
    s.sundial_shuffles = 2;
    let n = play(&s);
    assert_eq!(n.sundial_shuffles, 3);
    assert_eq!(n.player.energy, 5);
    let mut expected = s.rng.shuffle_rng.clone();
    expected.random_long();
    assert_eq!(
        serde_json::to_value(&n.rng.shuffle_rng).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&n.rng.card_random_rng).unwrap(),
        serde_json::to_value(&s.rng.card_random_rng).unwrap()
    );
    assert_eq!(n.piles.hand.len(), 2);
    assert!(n.player.cannot_draw);
}
#[test]
fn empty_shuffle_also_queues_abacus_block() {
    let mut s = fixture(2);
    s.player.authority.relics.push(Relic::TheAbacus);
    let n = play(&s);
    assert_eq!(n.player.block, 6);
}
#[test]
fn constructor_sundial_callback_precedes_void_on_draw_energy_loss() {
    let mut s = fixture(2);
    s.player.energy = 0;
    s.piles.draw_pile[1].content_id = VOID_ID;
    s.player.authority.relics.push(Relic::Sundial);
    s.sundial_shuffles = 2;
    let n = play(&s);
    assert_eq!(n.player.energy, 1); // +2, then Void -1; not floor(0-1)+2.
}
#[test]
fn no_overdraw_no_cards_or_no_draw_do_not_construct_empty_shuffle() {
    for (cards, no_draw) in [(3, false), (0, false), (2, true)] {
        let mut s = fixture(cards);
        s.player.cannot_draw = no_draw;
        s.player.authority.relics.push(Relic::Sundial);
        s.sundial_shuffles = 2;
        let n = play(&s);
        assert_eq!(n.sundial_shuffles, 2);
        assert_eq!(n.player.energy, 3);
        assert_eq!(
            serde_json::to_value(&n.rng).unwrap(),
            serde_json::to_value(&s.rng).unwrap()
        );
    }
}
