//! ExhaustAllNonAttackAction.update iterates hand forward, addToTop each
//! ExhaustSpecificCardAction. These synthetic controls supplement FIDL00022
//! step1580 and its later Exhume choice at1586; they are not trace substitutes.
use sts_core::adapter_internals::{
    apply_combat_action,
    combat::choose_exhaust_select,
    content::cards::{
        DAZED_ID, DEFEND_R_ID, EXHUME_ID, FIRE_BREATHING_ID, OFFERING_ID, POMMEL_STRIKE_ID,
        SEVER_SOUL_ID, STRIKE_R_ID,
    },
    CardId, CardInstance, CombatAction, CombatState,
};
fn fixture() -> CombatState {
    let mut s = CombatState::cultist_fixture();
    s.piles.hand.clear();
    s.piles.exhaust_pile.clear();
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.player.energy = 10;
    for (id, c) in [
        (99, SEVER_SOUL_ID),
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
fn nonattacks_exhaust_in_reverse_hand_order_not_forward() {
    let s = fixture();
    let n = play(&s, 99, true);
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
    assert_eq!(n.monsters[0].hp, s.monsters[0].hp - 16);
    assert_eq!(n.player.energy, 8);
    assert_eq!(n.piles.discard_pile[0].id, CardId::new(99));
    n.validate().unwrap();
}
#[test]
fn later_exhume_ordinal_addresses_the_resulting_physical_exhaust_order() {
    let s = play(&fixture(), 99, true);
    let s = play(&s, 7, true);
    let mut s = play(&s, 100, false);
    assert!(s.exhaust_select().is_some());
    choose_exhaust_select(&mut s, 3).unwrap();
    let selected = s.exhaust_select().unwrap().selected_hand_indices[0];
    assert_eq!(s.piles.exhaust_pile[selected].id, CardId::new(1));
    assert_eq!(s.piles.exhaust_pile[selected].content_id, OFFERING_ID);
}
