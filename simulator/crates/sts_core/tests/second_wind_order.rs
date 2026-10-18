//! PC BlockPerNonAttackAction queues forward with addToTop, so its physical
//! exhaust order is reversed, before the per-card GainBlockActions.
use sts_core::adapter_internals::{
    apply_combat_action,
    combat::choose_exhaust_select,
    content::cards::{
        DAZED_ID, DEFEND_R_ID, EXHUME_ID, FIRE_BREATHING_ID, OFFERING_ID, POMMEL_STRIKE_ID,
        SECOND_WIND_ID, SECOND_WIND_PLUS_ID, STRIKE_R_ID,
    },
    CardId, CardInstance, CombatAction, CombatState,
};
fn fixture(upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.piles.hand.clear();
    s.piles.exhaust_pile.clear();
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.player.energy = 10;
    for (id, c) in [
        (
            99,
            if upgraded {
                SECOND_WIND_PLUS_ID
            } else {
                SECOND_WIND_ID
            },
        ),
        (1, OFFERING_ID),
        (2, DAZED_ID),
        (3, STRIKE_R_ID),
        (4, FIRE_BREATHING_ID),
        (5, STRIKE_R_ID),
        (6, DEFEND_R_ID),
        (7, POMMEL_STRIKE_ID),
    ] {
        s.piles.hand.push(CardInstance::new(CardId::new(id), c));
    }
    s.piles
        .draw_pile
        .push(CardInstance::new(CardId::new(100), EXHUME_ID));
    s.validate().unwrap();
    s
}
fn play(s: &CombatState, id: u64, target: bool) -> CombatState {
    apply_combat_action(
        s,
        CombatAction::PlayCard {
            card_id: CardId::new(id),
            target: target.then_some(s.monsters[0].id),
        },
    )
    .unwrap()
}
#[test]
fn reverse_exhaust_order_preserves_card_count_block_and_attacks() {
    for upgraded in [false, true] {
        let s = fixture(upgraded);
        let n = play(&s, 99, false);
        assert_eq!(
            n.piles
                .exhaust_pile
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            [6, 4, 2, 1].map(CardId::new)
        );
        assert_eq!(
            n.piles.hand.iter().map(|c| c.id).collect::<Vec<_>>(),
            [3, 5, 7].map(CardId::new)
        );
        assert_eq!(n.player.block, 4 * if upgraded { 7 } else { 5 });
        assert_eq!(n.player.energy, 9);
        assert_eq!(n.piles.discard_pile[0].id, CardId::new(99));
        n.validate().unwrap();
    }
}
#[test]
fn subsequent_exhume_selects_from_natural_second_wind_exhaust_order() {
    let s = play(&fixture(false), 99, false);
    let s = play(&s, 7, true);
    let mut s = play(&s, 100, false);
    choose_exhaust_select(&mut s, 3).unwrap();
    let index = s.exhaust_select().unwrap().selected_hand_indices[0];
    assert_eq!(s.piles.exhaust_pile[index].id, CardId::new(1));
    assert_eq!(s.piles.exhaust_pile[index].content_id, OFFERING_ID);
}
