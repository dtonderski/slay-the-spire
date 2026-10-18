//! QLFB00042 step1450: lethal Conclude cancels its queued end-turn press.
//! These initialized controls supplement, not replace, that real trace.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::cards::{CONCLUDE_ANY_COLOR_ID, DECAY_ID, DEFEND_R_ID},
    CardId, CardInstance, CombatAction, CombatPhase, CombatState,
};
fn fixture(hp: i32, upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    let mut card = CardInstance::new(CardId::new(17), CONCLUDE_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    s.piles.hand = vec![card, CardInstance::new(CardId::new(18), DEFEND_R_ID)];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    s.monsters[0].hp = hp;
    s.monsters[0].max_hp = hp;
    s.player.block = 5;
    s.player.powers.metallicize = 3;
    s.player.powers.constricted = 10;
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
#[test]
fn lethal_conclude_does_not_invent_an_end_power_window() {
    for upgraded in [false, true] {
        let s = fixture(1, upgraded);
        let n = play(&s);
        assert_eq!(n.phase, CombatPhase::Won);
        assert_eq!(n.player.hp, s.player.hp);
        assert!(!n.time_warp_end_turn);
        assert_eq!(n.piles.hand.len(), 1);
        assert_eq!(n.piles.hand[0].id, CardId::new(18));
        assert_eq!(n.piles.discard_pile[0].id, CardId::new(17));
    }
}
#[test]
fn lethal_conclude_does_not_fire_decay_from_the_remaining_hand() {
    let mut s = fixture(1, false);
    s.piles.hand[1].content_id = DECAY_ID;
    let n = play(&s);
    assert_eq!(n.player.hp, s.player.hp);
    assert_eq!(n.piles.hand[0].content_id, DECAY_ID);
}

#[test]
fn nonlethal_conclude_still_ends_turn_and_runs_constricted() {
    let s = fixture(100, false);
    let n = play(&s);
    assert_eq!(n.player.hp, s.player.hp - 2); // existing5 + Metallicize3, then Constricted10
    assert!(n.monsters[0].alive);
    assert_ne!(
        serde_json::to_value(&n.rng.shuffle_rng).unwrap(),
        serde_json::to_value(&s.rng.shuffle_rng).unwrap()
    );
}
