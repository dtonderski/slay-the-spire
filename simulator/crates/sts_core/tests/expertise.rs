//! PC Expertise.use / ExpertiseAction.update / DrawCardAction.update rules.
//! Synthetic controls supplement FIDL01535 step747; they are not parity evidence.
use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{DEFEND_R_ID, EXPERTISE_ANY_COLOR_ID, PAIN_ID},
        shop_pool::shop_card_content_id,
    },
    legal_combat_actions, CardId, CardInstance, CombatAction, CombatState, Relic,
};

fn fixture(upgraded: bool, other_hand: usize) -> CombatState {
    let mut state = CombatState::cultist_fixture();
    state.piles.hand.clear();
    state.piles.draw_pile.clear();
    state.piles.discard_pile.clear();
    let mut expertise = CardInstance::new(CardId::new(99), EXPERTISE_ANY_COLOR_ID);
    expertise.upgrades = u8::from(upgraded);
    state.piles.hand.push(expertise);
    for i in 0..other_hand {
        state
            .piles
            .hand
            .push(CardInstance::new(CardId::new(20 + i as u64), DEFEND_R_ID));
    }
    for i in 0..10 {
        state
            .piles
            .draw_pile
            .push(CardInstance::new(CardId::new(200 + i), DEFEND_R_ID));
    }
    state.player.energy = 3;
    state.validate().unwrap();
    state
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
fn expertise_identity_and_base_upgraded_hand_targets() {
    assert_eq!(shop_card_content_id("EXPERTISE"), EXPERTISE_ANY_COLOR_ID);
    for upgraded in [false, true] {
        let state = fixture(upgraded, 1);
        let next = play(&state);
        next.validate().unwrap();
        let target = if upgraded { 7 } else { 6 };
        assert_eq!(next.piles.hand.len(), target);
        assert_eq!(next.piles.draw_pile.len(), 11 - target);
        assert_eq!(next.player.energy, 2);
        assert_eq!(next.piles.discard_pile.len(), 1);
        assert_eq!(next.piles.discard_pile[0].id, CardId::new(99));
    }
}
#[test]
fn expertise_is_legal_under_no_draw_but_does_not_draw_or_use_rng() {
    let mut state = fixture(false, 2);
    state.player.cannot_draw = true;
    let before = serde_json::to_value(&state).unwrap();
    assert!(legal_combat_actions(&state)
        .unwrap()
        .contains(&CombatAction::PlayCard {
            card_id: CardId::new(99),
            target: None
        }));
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    let next = play(&state);
    next.validate().unwrap();
    assert_eq!(next.piles.hand.len(), 2);
    assert_eq!(next.piles.draw_pile, state.piles.draw_pile);
    assert_eq!(
        serde_json::to_value(&next.rng).unwrap(),
        serde_json::to_value(&state.rng).unwrap()
    );
    assert_eq!(next.player.energy, 2);
    assert!(next.player.cannot_draw);
}
#[test]
fn expertise_does_not_draw_above_target() {
    for n in [6, 7, 8, 9] {
        let state = fixture(false, n);
        let next = play(&state);
        assert_eq!(next.piles.hand.len(), n);
        assert_eq!(next.piles.draw_pile, state.piles.draw_pile);
    }
}
#[test]
fn ink_bottle_draw_follows_expertise_fill_and_precedes_source_discard() {
    let mut state = fixture(false, 1);
    state.player.authority.relics.push(Relic::InkBottle);
    state.ink_bottle_cards_played = 9;
    let next = play(&state);
    next.validate().unwrap();
    // AbstractPlayer.useCard calls Expertise.use before constructing
    // UseCardAction, whose relic callback queues Ink Bottle's later draw.
    assert_eq!(next.piles.hand.len(), 7);
    assert_eq!(next.piles.draw_pile.len(), 4);
    assert_eq!(next.piles.discard_pile.len(), 1);
}
#[test]
fn expertise_reads_live_hand_after_earlier_pain_runic_cube_draw() {
    let mut state = fixture(false, 1);
    state
        .piles
        .hand
        .push(CardInstance::new(CardId::new(77), PAIN_ID));
    state.player.authority.relics.push(Relic::RunicCube);
    let next = play(&state);
    next.validate().unwrap();
    // Pain addToTop damage and Runic Cube's draw precede ExpertiseAction.
    assert_eq!(next.player.hp, state.player.hp - 1);
    assert_eq!(next.piles.hand.len(), 6);
    assert_eq!(next.piles.draw_pile.len(), 6);
}
#[test]
fn duplicated_expertise_uses_live_hand_and_pays_only_original_cost() {
    let mut state = fixture(false, 1);
    state.duplication_potion_pending = true;
    state.duplication_potion_stacks = 1;
    let next = play(&state);
    next.validate().unwrap();
    assert_eq!(next.piles.hand.len(), 6);
    assert_eq!(next.piles.draw_pile.len(), 5);
    assert_eq!(next.player.energy, 2);
    assert_eq!(
        next.piles
            .discard_pile
            .iter()
            .filter(|c| c.content_id == EXPERTISE_ANY_COLOR_ID)
            .count(),
        1
    );
}
