//! Source-backed controls supplement immutable FIDL01243 step827.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{upgrade_card_instance, DEFEND_R_ID, STACK_ANY_COLOR_ID},
        shop_pool::shop_card_content_id,
    },
    CardId, CardInstance, CombatAction, CombatState, Relic,
};
fn fixture(discard: u64, upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    let mut card = CardInstance::new(CardId::new(17), STACK_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    s.piles.hand = vec![card];
    s.piles.draw_pile.clear();
    s.piles.discard_pile = (0..discard)
        .map(|i| CardInstance::new(CardId::new(30 + i), DEFEND_R_ID))
        .collect();
    s.piles.exhaust_pile.clear();
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
fn stack_registry_preserves_id_and_native_upgrade() {
    assert_eq!(shop_card_content_id("STACK"), STACK_ANY_COLOR_ID);
    let card = CardInstance::new(CardId::new(17), STACK_ANY_COLOR_ID);
    let upgraded = upgrade_card_instance(card).unwrap().unwrap();
    assert_eq!(upgraded.content_id, STACK_ANY_COLOR_ID);
    assert_eq!(upgraded.upgrades, 1);
    assert!(upgrade_card_instance(upgraded).unwrap().is_none());
}
#[test]
fn base_and_upgraded_count_discard_without_counting_source() {
    for discard in [0, 3, 8] {
        for upgraded in [false, true] {
            let s = fixture(discard, upgraded);
            let n = play(&s);
            assert_eq!(
                n.player.block,
                discard as i32 + if upgraded { 3 } else { 0 }
            );
            assert_eq!(n.player.energy, s.player.energy - 1);
            assert_eq!(n.piles.discard_pile.len(), discard as usize + 1);
            assert_eq!(n.piles.discard_pile.last().unwrap().id, CardId::new(17));
            assert_eq!(
                serde_json::to_value(n.rng).unwrap(),
                serde_json::to_value(s.rng).unwrap()
            );
        }
    }
}
#[test]
fn dexterity_frail_and_no_block_apply_at_preparation() {
    let mut s = fixture(7, true);
    s.player.powers.dexterity = 2;
    s.player.powers.frail = 1;
    assert_eq!(play(&s).player.block, 9); // floor((7+3+2)*.75)
    s.player.no_block_turns = 1;
    assert_eq!(play(&s).player.block, 0);
    s.player.cannot_draw = true;
    assert_eq!(play(&s).player.energy, s.player.energy - 1);
}
#[test]
fn copy_recalculates_after_original_enters_discard() {
    for upgraded in [false, true] {
        let mut s = fixture(3, upgraded);
        s.duplication_potion_pending = true;
        s.duplication_potion_stacks = 1;
        let n = play(&s);
        assert_eq!(n.player.block, if upgraded { 13 } else { 7 });
        assert_eq!(n.player.energy, s.player.energy - 1);
        assert_eq!(n.piles.discard_pile.len(), 4);
    }
}
#[test]
fn original_use_snapshots_before_ink_bottle_reshuffle() {
    let mut s = fixture(3, false);
    s.player.authority.relics.push(Relic::InkBottle);
    s.ink_bottle_cards_played = 9;
    let n = play(&s);
    assert_eq!(n.player.block, 3);
    assert_eq!(n.piles.hand.len(), 1);
    assert_eq!(n.piles.discard_pile.len(), 1);
}
#[test]
fn copied_use_snapshots_before_its_own_ink_bottle_reshuffle() {
    let mut s = fixture(3, false);
    s.player.authority.relics.push(Relic::InkBottle);
    s.ink_bottle_cards_played = 8;
    s.duplication_potion_pending = true;
    s.duplication_potion_stacks = 1;
    let n = play(&s);
    assert_eq!(n.player.block, 7); // original3, copy4 before its draw empties discard
    assert_eq!(n.piles.hand.len(), 1);
    assert!(n.piles.discard_pile.is_empty());
}
#[test]
fn clone_and_restore_preserve_copied_use_rng_and_result() {
    let mut s = fixture(3, true);
    s.player.authority.relics.push(Relic::InkBottle);
    s.ink_bottle_cards_played = 8;
    s.duplication_potion_pending = true;
    s.duplication_potion_stacks = 1;
    let restored: CombatState = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
    assert_eq!(play(&s), play(&restored));
}
