use sts_core::adapter_internals::{
    apply_combat_action, content::cards::STREAMLINE_ANY_COLOR_ID, CardId, CardInstance,
    CombatAction, CombatState, Relic,
};

fn check_streamline_copy(copy_source: &str) {
    // Reduced from synthetic fuzz seed 469. Necronomicon.onUseCard uses
    // makeSameInstanceOf (same UUID); Streamline.use queues ReduceCostAction,
    // whose GetAllInBattleInstances lookup includes discard, exhaust, and limbo.
    // The original has settled into discard when the purgeOnUse copy resolves.
    let mut combat = CombatState::cultist_fixture();
    match copy_source {
        "necronomicon" => combat.player.authority.relics = vec![Relic::Necronomicon],
        "double_tap" => combat.double_tap_pending = 1,
        "duplication" => {
            combat.duplication_potion_pending = true;
            combat.duplication_potion_stacks = 1;
        }
        _ => unreachable!(),
    }
    combat.player.energy = 3;
    combat.monsters[0].hp = 100;
    combat.monsters[0].max_hp = 100;
    combat.piles.hand = vec![CardInstance::new(CardId::new(12), STREAMLINE_ANY_COLOR_ID)];
    combat.piles.draw_pile.clear();
    combat.validate().expect("valid initial combat");
    let next = apply_combat_action(
        &combat,
        CombatAction::PlayCard {
            card_id: CardId::new(12),
            target: Some(combat.monsters[0].id),
        },
    )
    .expect("enumerated Streamline play resolves both uses");
    next.validate().expect("valid successor");
    assert_eq!(next.player.energy, 1);
    assert_eq!(next.monsters[0].hp, 70);
    assert_eq!(next.piles.discard_pile.len(), 1);
    assert_eq!(next.piles.discard_pile[0].id, CardId::new(12));
    assert_eq!(next.piles.discard_pile[0].temp_cost, Some(0));
}

#[test]
fn necronomicon_streamline_reduces_original_in_discard() {
    check_streamline_copy("necronomicon");
}

#[test]
fn double_tap_streamline_reduces_original_in_discard() {
    check_streamline_copy("double_tap");
}

#[test]
fn duplication_streamline_reduces_original_in_discard() {
    check_streamline_copy("duplication");
}
