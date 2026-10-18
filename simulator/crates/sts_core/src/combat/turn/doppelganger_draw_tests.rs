use super::*;
use crate::{
    card::CardInstance,
    content::cards::{BASH_ID, DEFEND_R_ID, STRIKE_R_ID},
    ids::CardId,
};
#[test]
fn next_turn_draw_precedes_mayhem_extraction_but_its_evolve_callbacks_follow_it() {
    let mut s = CombatState::cultist_fixture();
    s.player.authority.relics.clear();
    s.monsters[0].hp = 500;
    s.monsters[0].max_hp = 500;
    s.piles.hand.clear();
    s.piles.discard_pile.clear();
    s.piles.exhaust_pile.clear();
    s.player.powers.draw_card = 1;
    s.player.powers.evolve = 1;
    s.player.powers.mayhem = 1;
    let ids = [
        DEFEND_R_ID,
        BASH_ID,
        STRIKE_R_ID,
        DEFEND_R_ID,
        DEFEND_R_ID,
        DEFEND_R_ID,
        DEFEND_R_ID,
        DEFEND_R_ID,
        WOUND_ID,
    ];
    s.piles.draw_pile = ids
        .iter()
        .enumerate()
        .map(|(i, id)| CardInstance::new(CardId::new(50 + i as u64), *id))
        .collect();
    start_player_turn(&mut s).unwrap();
    assert_eq!(s.monsters[0].hp, 494);
    assert_eq!(s.piles.hand.len(), 7);
    assert!(s.piles.hand.iter().any(|c| c.content_id == BASH_ID));
    assert!(!s.piles.hand.iter().any(|c| c.content_id == STRIKE_R_ID));
    assert!(s
        .piles
        .discard_pile
        .iter()
        .any(|c| c.content_id == STRIKE_R_ID));
    assert_eq!(s.player.powers.draw_card, 0);
}
#[test]
fn no_draw_skips_bonus_but_removes_power_and_does_not_draw_rng() {
    let mut s = CombatState::cultist_fixture();
    s.player.powers.draw_card = 3;
    s.player.cannot_draw = true;
    let before_rng = s.rng.clone();
    let before_hand = s.piles.hand.clone();
    assert!(apply_draw_card_next_turn_post_draw(&mut s)
        .unwrap()
        .is_empty());
    assert_eq!(s.player.powers.draw_card, 0);
    assert_eq!(s.rng, before_rng);
    assert_eq!(s.piles.hand, before_hand);
}
