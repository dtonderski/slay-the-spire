use sts_core::adapter_internals::{
    apply_combat_action,
    content::cards::{DEFEND_R_ID, STRIKE_R_ID, UNLOAD_ANY_COLOR_ID},
    CardId, CardInstance, CombatAction, CombatState, Relic,
};

fn check_copy(copy_source: &str, ink_bottle: bool) {
    // Reduced from fuzz seed 26866: each UnloadAction.update selects the live
    // non-attack hand after damage, not the original use's discard IDs.
    let mut combat = CombatState::cultist_fixture();
    let mut unload = CardInstance::new(CardId::new(99), UNLOAD_ANY_COLOR_ID);
    unload.temp_cost = Some(2);
    match copy_source {
        "necronomicon" => combat.player.authority.relics = vec![Relic::Necronomicon],
        "double_tap" => combat.double_tap_pending = 1,
        "duplication" => {
            combat.duplication_potion_pending = true;
            combat.duplication_potion_stacks = 1;
        }
        _ => unreachable!(),
    }
    if ink_bottle {
        combat.player.authority.relics.push(Relic::InkBottle);
        combat.ink_bottle_cards_played = 9;
    }
    combat.player.energy = 3;
    combat.monsters[0].hp = 100;
    combat.monsters[0].max_hp = 100;
    combat.piles.hand = vec![
        unload,
        CardInstance::new(CardId::new(20), DEFEND_R_ID),
        CardInstance::new(CardId::new(21), DEFEND_R_ID),
        CardInstance::new(CardId::new(30), STRIKE_R_ID),
    ];
    combat.piles.draw_pile.clear();
    if ink_bottle {
        combat
            .piles
            .draw_pile
            .push(CardInstance::new(CardId::new(55), DEFEND_R_ID));
    }
    combat.validate().expect("initial");
    let next = apply_combat_action(
        &combat,
        CombatAction::PlayCard {
            card_id: unload.id,
            target: Some(combat.monsters[0].id),
        },
    )
    .expect("both Unload uses resolve");
    next.validate().expect("successor");
    assert_eq!(next.monsters[0].hp, 72);
    assert_eq!(next.player.energy, 1);
    assert_eq!(
        next.total_discarded_this_turn,
        if ink_bottle { 3 } else { 2 }
    );
    if ink_bottle {
        assert!(next
            .piles
            .discard_pile
            .iter()
            .any(|card| card.id == CardId::new(55)));
    }
    assert_eq!(
        next.piles
            .hand
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![CardId::new(30)]
    );
    assert_eq!(
        next.piles
            .discard_pile
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        if ink_bottle {
            vec![
                CardId::new(21),
                CardId::new(20),
                CardId::new(99),
                CardId::new(55),
            ]
        } else {
            vec![CardId::new(21), CardId::new(20), CardId::new(99)]
        }
    );
}

#[test]
fn necronomicon_unload_does_not_discard_original_targets_twice() {
    check_copy("necronomicon", false);
}
#[test]
fn double_tap_unload_does_not_discard_original_targets_twice() {
    check_copy("double_tap", false);
}
#[test]
fn duplication_unload_does_not_discard_original_targets_twice() {
    check_copy("duplication", false);
}

#[test]
fn copied_unload_discards_new_non_attack_drawn_between_uses() {
    check_copy("necronomicon", true);
}
