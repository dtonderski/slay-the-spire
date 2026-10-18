//! Distraction.use reserves a typed Skill before callbacks; source exhausts.
//! Supplemental controls; natural witness QLFB00042 step1425.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{upgrade_card_instance, DEFEND_R_ID, DISTRACTION_ANY_COLOR_ID},
        shop_pool::{
            ironclad_combat_discovery_pool, ironclad_combat_skill_discovery_pool,
            shop_card_content_id,
        },
    },
    CardId, CardInstance, CombatAction, CombatState, ContentId, Relic,
};
fn fixture(upgraded: bool) -> CombatState {
    let mut s = CombatState::cultist_fixture();
    let mut card = CardInstance::new(CardId::new(17), DISTRACTION_ANY_COLOR_ID);
    card.upgrades = u8::from(upgraded);
    s.piles.hand = vec![card];
    s.piles.draw_pile.clear();
    s.piles.discard_pile.clear();
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
fn skill(s: &mut CombatState) -> ContentId {
    let pool = ironclad_combat_skill_discovery_pool();
    pool[s.rng.card_random_rng.random_int((pool.len() - 1) as i32) as usize]
}
fn assert_rng(s: &CombatState, n: &CombatState) {
    assert_eq!(
        serde_json::to_value(&s.rng.card_random_rng).unwrap(),
        serde_json::to_value(&n.rng.card_random_rng).unwrap()
    );
}
#[test]
fn native_id_upgrade_cost_and_exhaust_are_preserved() {
    assert_eq!(
        shop_card_content_id("DISTRACTION"),
        DISTRACTION_ANY_COLOR_ID
    );
    assert_eq!(
        upgrade_card_instance(fixture(false).piles.hand[0])
            .unwrap()
            .unwrap()
            .upgrades,
        1
    );
    for upgraded in [false, true] {
        let s = fixture(upgraded);
        let mut expected = s.clone();
        let id = skill(&mut expected);
        let n = play(&s);
        assert_eq!(
            n.player.energy,
            s.player.energy - if upgraded { 0 } else { 1 }
        );
        assert_eq!(n.piles.hand.len(), 1);
        assert_eq!(n.piles.hand[0].content_id, id);
        assert_eq!(n.piles.hand[0].temp_cost, Some(0));
        assert!(n.piles.hand[0].temp_cost_turn_only);
        assert_eq!(n.piles.exhaust_pile.len(), 1);
        assert_eq!(n.piles.exhaust_pile[0].id, CardId::new(17));
        assert_rng(&expected, &n);
    }
}
#[test]
fn zero_energy_upgraded_and_no_draw_still_generate() {
    let mut s = fixture(true);
    s.player.energy = 0;
    s.player.cannot_draw = true;
    assert_eq!(play(&s).piles.hand.len(), 1);
}
#[test]
fn played_source_does_not_occupy_a_hand_slot() {
    let mut s = fixture(false);
    s.piles
        .hand
        .extend((30..39).map(|i| CardInstance::new(CardId::new(i), DEFEND_R_ID)));
    let n = play(&s);
    assert_eq!(n.piles.hand.len(), 10);
    assert!(n.piles.discard_pile.is_empty());
}
#[test]
fn copied_use_reserves_a_new_skill_and_overflows_without_reexhausting_source() {
    let mut s = fixture(true);
    s.piles
        .hand
        .extend((30..39).map(|i| CardInstance::new(CardId::new(i), DEFEND_R_ID)));
    s.duplication_potion_pending = true;
    s.duplication_potion_stacks = 1;
    let mut expected = s.clone();
    let first = skill(&mut expected);
    let second = skill(&mut expected);
    let n = play(&s);
    assert_eq!(n.piles.hand[9].content_id, first);
    assert_eq!(n.piles.discard_pile.len(), 1);
    assert_eq!(n.piles.discard_pile[0].content_id, second);
    assert_eq!(n.piles.exhaust_pile.len(), 1);
    assert_rng(&expected, &n);
}
#[test]
fn dead_branch_source_callback_precedes_copied_skill_roll() {
    let mut s = fixture(false);
    s.player.authority.relics.push(Relic::DeadBranch);
    s.duplication_potion_pending = true;
    s.duplication_potion_stacks = 1;
    let mut expected = s.clone();
    let first = skill(&mut expected);
    let pool = ironclad_combat_discovery_pool();
    let branch = pool[expected
        .rng
        .card_random_rng
        .random_int((pool.len() - 1) as i32) as usize];
    let second = skill(&mut expected);
    let n = play(&s);
    assert_eq!(
        n.piles
            .hand
            .iter()
            .map(|c| c.content_id)
            .collect::<Vec<_>>(),
        vec![first, branch, second]
    );
    assert_rng(&expected, &n);
}
#[test]
fn copied_use_roll_precedes_its_ink_bottle_confusion_draw() {
    let mut s = fixture(false);
    s.player.authority.relics.push(Relic::InkBottle);
    s.ink_bottle_cards_played = 8;
    s.player.powers.confusion = 1;
    s.piles.draw_pile = vec![CardInstance::new(CardId::new(90), DEFEND_R_ID)];
    s.duplication_potion_pending = true;
    s.duplication_potion_stacks = 1;
    let mut expected = s.clone();
    let first = skill(&mut expected);
    let second = skill(&mut expected);
    expected.rng.card_random_rng.random_int(3);
    let n = play(&s);
    let generated = n
        .piles
        .hand
        .iter()
        .filter(|c| c.id != CardId::new(90))
        .map(|c| c.content_id)
        .collect::<Vec<_>>();
    assert_eq!(generated, vec![first, second]);
    assert_rng(&expected, &n);
}
#[test]
fn generated_zero_cost_expires_and_clone_restore_is_deterministic() {
    let s = fixture(false);
    let restored: CombatState = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
    let n = play(&s);
    assert_eq!(n, play(&restored));
    let id = n.piles.hand[0].id;
    let after = apply_combat_action(&n, CombatAction::EndTurn).unwrap();
    let card = after
        .piles
        .hand
        .iter()
        .chain(&after.piles.draw_pile)
        .chain(&after.piles.discard_pile)
        .find(|c| c.id == id)
        .unwrap();
    assert!(!card.temp_cost_turn_only);
    assert_eq!(card.temp_cost, None);
}
