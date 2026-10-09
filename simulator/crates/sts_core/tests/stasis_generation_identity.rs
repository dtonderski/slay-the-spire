use sts_core::adapter_internals::{
    apply_combat_action,
    content::{
        cards::{DEFEND_R_ID, IMMOLATE_ID, WILD_STRIKE_ID, WILD_STRIKE_PLUS_ID, WOUND_ID},
        monsters::{monster_state, BRONZE_ORB_A0, JAW_WORM_A0},
    },
    CardId, CardInstance, CombatAction, CombatState, ContentId, MonsterId,
};

fn scenario(content: ContentId) -> CombatState {
    let mut state = CombatState::initial_fixture();
    state.monsters = vec![
        monster_state(&BRONZE_ORB_A0, MonsterId::new(1)),
        monster_state(&JAW_WORM_A0, MonsterId::new(2)),
    ];
    state.monsters[0].hp = 1;
    state.monsters[0].stasis_card = Some(CardInstance::new(CardId::new(99), IMMOLATE_ID));
    state.piles.hand = vec![CardInstance::new(CardId::new(1), content)];
    state.piles.draw_pile = vec![CardInstance::new(CardId::new(98), DEFEND_R_ID)];
    state.piles.discard_pile.clear();
    state.piles.exhaust_pile.clear();
    state.validate().expect("valid synthetic setup");
    state
}

fn check_lethal_generation(content: ContentId) {
    // Reduced from synthetic wide-endurance seed 142376. Lethal damage queues
    // the Stasis return after Wild Strike's status generation. Physical card
    // ownership must reserve the held ID through that queue window.
    let state = scenario(content);
    let action = CombatAction::PlayCard {
        card_id: CardId::new(1),
        target: Some(MonsterId::new(1)),
    };
    let restored: CombatState =
        serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    let next = apply_combat_action(&state, action).expect("lethal Wild Strike accepts");
    next.validate()
        .expect("return and generated status have distinct IDs");
    assert_eq!(next, apply_combat_action(&restored, action).unwrap());
    assert!(!next.monsters[0].alive);
    assert!(next.monsters[0].stasis_card.is_none());
    assert_eq!(
        next.piles.hand,
        vec![CardInstance::new(CardId::new(99), IMMOLATE_ID)]
    );
    let wound = next
        .piles
        .draw_pile
        .iter()
        .find(|card| card.content_id == WOUND_ID)
        .unwrap();
    assert!(
        wound.id.get() > 99,
        "queued physical instance remains reserved"
    );
    assert!(wound.combat_only);
    assert_eq!(
        next.piles.discard_pile,
        vec![CardInstance::new(CardId::new(1), content)]
    );
}

#[test]
fn lethal_wild_strike_does_not_reuse_queued_stasis_id() {
    check_lethal_generation(WILD_STRIKE_ID);
}

#[test]
fn lethal_upgraded_wild_strike_does_not_reuse_queued_stasis_id() {
    check_lethal_generation(WILD_STRIKE_PLUS_ID);
}
